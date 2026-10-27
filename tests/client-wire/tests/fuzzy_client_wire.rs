//! Arbitrary and mutated client bytes (skein's codec.md, section 6).
use jig_client_protocol_wire::v19::*;
use skein_lib::{Reader, Rng, Writer};
use std::{fs, path::Path};

macro_rules! check {
    ($record:ident, $bytes:expr) => {
        if let Ok(record) = $record::decode(&CEILINGS, &mut Reader::new($bytes)) {
            let mut writer = Writer::new(usize::try_from(record.measure()).expect("measured length"));
            record.encode(&mut writer).expect("measured room");
            assert_eq!(writer.finish().as_ref(), $bytes, stringify!($record));
        }
    };
}

#[path = "fuzzy_client_wire/documents.rs"]
mod documents;

fn records_1(bytes: &[u8]) {
    check!(ReadValue, bytes);
    check!(DeliveryValue, bytes);
    check!(StreamText, bytes);
    check!(Chips, bytes);
    check!(OwnerInbox, bytes);
    check!(Kept, bytes);
    check!(TableEntry, bytes);
    check!(Family, bytes);
    check!(Owner, bytes);
    check!(Keyed, bytes);
    check!(Owned, bytes);
    check!(KeyedAnswer, bytes);
    check!(LinkSignIn, bytes);
    check!(SignedIn, bytes);
    check!(SignInAnswer, bytes);
    check!(SignOut, bytes);
    check!(SignOutAnswer, bytes);
    check!(Authorize, bytes);
    check!(AuthorizeGo, bytes);
    check!(AuthorizeWhy, bytes);
    check!(AuthorizeRefused, bytes);
    check!(AuthorizeAnswer, bytes);
    check!(DocumentText, bytes);
    check!(ProposeSettings, bytes);
    check!(OriginRevision, bytes);
    check!(ReviewItem, bytes);
    check!(SettingsReview, bytes);
    check!(OwnerProposalActionSettings, bytes);
    check!(OwnerRetainedActionSettings, bytes);
    check!(SettingOrigin, bytes);
    check!(ReviewValue, bytes);

    check!(ExecutorAgent, bytes);
    check!(ExecutorProcedure, bytes);
    check!(ExecutorParty, bytes);
    check!(ExecutorRole, bytes);
    check!(Executor, bytes);
    check!(Phase, bytes);
    check!(RunStageClaimed, bytes);
    check!(RunStageRunning, bytes);
    check!(RunStageBackingOff, bytes);
    check!(RunStage, bytes);
    check!(PersonAddressParty, bytes);
    check!(PersonAddressRole, bytes);
    check!(PersonAddress, bytes);
    check!(ClosingStageRun, bytes);
    check!(ClosingStage, bytes);
    check!(HoldFailure, bytes);
    check!(HoldCategoryFailures, bytes);
    check!(HoldCategoryDependency, bytes);
    check!(HoldCategoryPlaced, bytes);
    check!(HoldCategoryUncertain, bytes);
    check!(HoldCategoryPoolLost, bytes);
    check!(HoldCategory, bytes);
    check!(OwnerChipPhaseWaiting, bytes);
    check!(OwnerChipPhaseActive, bytes);
    check!(OwnerChipPhasePerson, bytes);
    check!(OwnerChipPhaseClosing, bytes);
    check!(OwnerChipPhaseHeld, bytes);
    check!(OwnerChipPhase, bytes);
    check!(FailureClass, bytes);
}

fn records_2(bytes: &[u8]) {
    check!(RetryNoticeOwnerFailed, bytes);
    check!(RetryNoticeFailed, bytes);
    check!(RetryNoticeRefused, bytes);
    check!(RetryNotice, bytes);
    check!(CallIdentity, bytes);
    check!(RunWorkTool, bytes);
    check!(RunWork, bytes);
    check!(RunObservation, bytes);
    check!(ActiveRun, bytes);
    check!(ProposerTask, bytes);
    check!(ProposerParty, bytes);
    check!(Proposer, bytes);
    check!(ProposalKey, bytes);
    check!(PartScopeProject, bytes);
    check!(PartScope, bytes);
    check!(PartConnector, bytes);
    check!(PartResource, bytes);
    check!(PartKind, bytes);
    check!(PartPanel, bytes);
    check!(PartResult, bytes);
    check!(PartEffect, bytes);
    check!(PartOwnerEffect, bytes);
    check!(PartOutbox, bytes);
    check!(PartHold, bytes);
    check!(PartProject, bytes);
    check!(PartDeployment, bytes);
    check!(Part, bytes);
    check!(WakePolicy, bytes);
    check!(RecipientTask, bytes);
    check!(RecipientParty, bytes);
    check!(RecipientRole, bytes);
    check!(RecipientRoles, bytes);
    check!(Recipient, bytes);
    check!(TakenBy, bytes);
    check!(Revision, bytes);
    check!(ActorParty, bytes);
    check!(ActorTask, bytes);
    check!(ActorProjectDeployment, bytes);
    check!(Actor, bytes);
    check!(PartyKind, bytes);
}

fn records_3(bytes: &[u8]) {
    check!(Party, bytes);
    check!(DecisionRejected, bytes);
    check!(DecisionReleased, bytes);
    check!(DecisionLeftHeld, bytes);
    check!(Decision, bytes);
    check!(AttemptFailure, bytes);
    check!(OwnerExecutorAgent, bytes);
    check!(OwnerExecutorProcedure, bytes);
    check!(OwnerExecutorPerson, bytes);
    check!(OwnerExecutor, bytes);
    check!(WakeRuleBatch, bytes);
    check!(WakeRule, bytes);
    check!(ResultsWake, bytes);
    check!(OwnerWakePolicy, bytes);
    check!(OwnerPersonStallNotice, bytes);
    check!(OwnerPersonWait, bytes);
    check!(OwnerPartKind, bytes);
    check!(OwnerPartKeyPath, bytes);
    check!(OwnerPartKeyNumber, bytes);
    check!(OwnerPartKey, bytes);
    check!(OwnerPart, bytes);
    check!(OwnerPriorPhaseActive, bytes);
    check!(OwnerPriorPhaseClosing, bytes);
    check!(OwnerPriorPhase, bytes);
    check!(OwnerEscalationHolderTask, bytes);
    check!(OwnerEscalationHolderPerson, bytes);
    check!(OwnerEscalationHolderRole, bytes);
    check!(OwnerEscalationHolder, bytes);
    check!(OwnerHoldEscalationWaiting, bytes);
    check!(OwnerHoldEscalationRejected, bytes);
    check!(OwnerHoldEscalation, bytes);
    check!(OwnerFailureTries, bytes);
    check!(OwnerChipHold, bytes);
    check!(OwnerNumbers, bytes);
    check!(FunderTask, bytes);
    check!(FunderPool, bytes);
    check!(FunderRecurring, bytes);
    check!(FunderPeriod, bytes);
    check!(Funder, bytes);
    check!(OwnerBudget, bytes);
}

fn records_4(bytes: &[u8]) {
    check!(OwnerResourceWriterRun, bytes);
    check!(OwnerResourceWriterEffect, bytes);
    check!(OwnerResourceWriter, bytes);
    check!(OwnerResourceLine, bytes);
    check!(OwnerChipWorkTool, bytes);
    check!(OwnerChipWork, bytes);
    check!(OwnerChipObservation, bytes);
    check!(OwnerRequesterTask, bytes);
    check!(OwnerRequesterPerson, bytes);
    check!(OwnerRequesterDeployment, bytes);
    check!(OwnerRequester, bytes);
    check!(ResultField, bytes);
    check!(ChipFields, bytes);
    check!(ChipRow, bytes);
    check!(ChipPhaseOwnerFields, bytes);
    check!(ChipPhaseActiveFields, bytes);
    check!(ChipPhaseWaitingFields, bytes);
    check!(ChipPhaseIdleFields, bytes);
    check!(ChipPhaseRunningFields, bytes);
    check!(ChipPhaseWaitingForWordsFields, bytes);
    check!(ChipPhaseBackingOffFields, bytes);
    check!(ChipPhaseProcedureFields, bytes);
    check!(ChipPhasePersonFields, bytes);
    check!(ChipPhaseClosingFields, bytes);
    check!(ChipPhaseHeldFields, bytes);
    check!(ChipPhaseEndedFields, bytes);
    check!(ChipPhaseFields, bytes);
    check!(ChipPhaseRow, bytes);
    check!(ClosingFields, bytes);
    check!(ClosingRow, bytes);
    check!(EndingTaskFields, bytes);
    check!(EndingFailedFields, bytes);
    check!(EndingCancelledFields, bytes);
    check!(EndingWithdrawnFields, bytes);
    check!(EndingDecidedFields, bytes);
    check!(EndingFields, bytes);
    check!(EndingRow, bytes);
    check!(HeldRuntimeActiveFields, bytes);
    check!(HeldRuntimeClosingFields, bytes);
    check!(HeldRuntimeFields, bytes);
}

fn records_5(bytes: &[u8]) {
    check!(HeldRuntimeRow, bytes);
    check!(HoldReasonTriesFields, bytes);
    check!(HoldReasonStalledFields, bytes);
    check!(HoldReasonDependencyFields, bytes);
    check!(HoldReasonDriftFields, bytes);
    check!(HoldReasonEffectFailedFields, bytes);
    check!(HoldReasonEffectUncertainFields, bytes);
    check!(HoldReasonBudgetFields, bytes);
    check!(HoldReasonDeadlineFields, bytes);
    check!(HoldReasonStoppedFields, bytes);
    check!(HoldReasonResourceFields, bytes);
    check!(HoldReasonPoolFields, bytes);
    check!(HoldReasonProcedureFields, bytes);
    check!(HoldReasonFields, bytes);
    check!(HoldReasonRow, bytes);
    check!(OwnerChipDetailsFields, bytes);
    check!(OwnerChipDetailsRow, bytes);
    check!(OwnerChipEndingDoneFields, bytes);
    check!(OwnerChipEndingFailedFields, bytes);
    check!(OwnerChipEndingCancelledFields, bytes);
    check!(OwnerChipEndingFields, bytes);
    check!(OwnerChipEndingRow, bytes);
    check!(ResultValueReportFields, bytes);
    check!(ResultValueOwnerVerdictFields, bytes);
    check!(ResultValueVerdictFields, bytes);
    check!(ResultValueConnectorFields, bytes);
    check!(ResultValueChangeFields, bytes);
    check!(ResultValueChoiceFields, bytes);
    check!(ResultValueApprovedFields, bytes);
    check!(ResultValueFailureFields, bytes);
    check!(ResultValueWordsFields, bytes);
    check!(ResultValueFields, bytes);
    check!(ResultValueRow, bytes);
    check!(Fact, bytes);
    check!(Chip, bytes);
    check!(ChipPhase, bytes);
    check!(Closing, bytes);
    check!(Ending, bytes);
    check!(HeldRuntime, bytes);
    check!(HoldReason, bytes);
}

fn records_6(bytes: &[u8]) {
    check!(OwnerChipDetails, bytes);
    check!(OwnerChipEnding, bytes);
    check!(ResultValue, bytes);
    check!(PersonAnswerChoice, bytes);
    check!(PersonAnswerApproved, bytes);
    check!(PersonAnswerWords, bytes);
    check!(PersonAnswerCannot, bytes);
    check!(PersonAnswer, bytes);
    check!(AddressChange, bytes);
    check!(ParameterNumber, bytes);
    check!(ParameterBytes, bytes);
    check!(ParameterResource, bytes);
    check!(Parameter, bytes);
    check!(Specification, bytes);
    check!(OwnerPatternEndExact, bytes);
    check!(OwnerPatternEndOpen, bytes);
    check!(OwnerPatternEnd, bytes);
    check!(OwnerPattern, bytes);
    check!(OwnerGrant, bytes);
    check!(OwnerAuthorityExecutorCharter, bytes);
    check!(OwnerAuthorityExecutorProcedure, bytes);
    check!(OwnerAuthorityExecutorRole, bytes);
    check!(OwnerAuthorityExecutor, bytes);
    check!(OwnerDelegation, bytes);
    check!(OwnerResourceScope, bytes);
    check!(OwnerAuthority, bytes);
    check!(Amendment, bytes);
    check!(SeenResults, bytes);
    check!(SeenConversation, bytes);
    check!(SeenThrough, bytes);
    check!(Seen, bytes);
    check!(NoteScopeProject, bytes);
    check!(NoteScopeResource, bytes);
    check!(NoteScopeTask, bytes);
}

fn records_7(bytes: &[u8]) {
    check!(NoteScope, bytes);
    check!(PolicyEditRole, bytes);
    check!(PolicyJudge, bytes);
    check!(PolicyGuardObserved, bytes);
    check!(PolicyGuard, bytes);
    check!(PolicyRequirement, bytes);
    check!(PolicyPermission, bytes);
    check!(PolicyEditProjectSpend, bytes);
    check!(PolicyEditRoleValue, bytes);
    check!(PolicyEditRequirements, bytes);
    check!(PolicyEditProjections, bytes);
    check!(PolicyEditPermissions, bytes);
    check!(PolicyEdit, bytes);
    check!(HeldRolePolicy, bytes);
    check!(HeldRole, bytes);
    check!(Holding, bytes);
    check!(Message, bytes);
    check!(AnswerQuestion, bytes);
    check!(AnswerPerson, bytes);
    check!(Take, bytes);
    check!(HandBack, bytes);
    check!(DecideProposal, bytes);
    check!(DecideEscalation, bytes);
    check!(Release, bytes);
    check!(Amend, bytes);
    check!(Stop, bytes);
    check!(Cancel, bytes);
    check!(Move, bytes);
    check!(Decide, bytes);
    check!(DecideOn, bytes);
    check!(Answer, bytes);
    check!(AnswerOn, bytes);
    check!(Tracking, bytes);
    check!(TrackingTracked, bytes);
    check!(TemplateCharter, bytes);
    check!(TemplateExecutor, bytes);
    check!(TemplateAmount, bytes);
    check!(TemplateAsAllowed, bytes);
    check!(TemplateBudget, bytes);
    check!(TemplateSpan, bytes);
    check!(TemplateDeadline, bytes);
    check!(TemplateOverride, bytes);
    check!(PolicyEditTemplates, bytes);
    check!(EditNote, bytes);
    check!(DeleteNote, bytes);
    check!(ChangePolicy, bytes);
    check!(SetRoles, bytes);
    check!(MakeService, bytes);
    check!(Rename, bytes);
    check!(MakeProject, bytes);
    check!(SetPool, bytes);
    check!(TaskRefMember, bytes);
    check!(TaskRefTask, bytes);
    check!(ContractReport, bytes);
    check!(ContractVerdict, bytes);
    check!(ContractChange, bytes);
    check!(BudgetAsAllowed, bytes);
    check!(Create, bytes);
    check!(ProposeBatch, bytes);
    check!(ProposeAmendment, bytes);
    check!(ProposeAction, bytes);
    check!(Propose, bytes);
    check!(Withdraw, bytes);
    check!(NewTask, bytes);
    check!(TaskRef, bytes);
    check!(NewContract, bytes);
    check!(NewBudget, bytes);
    check!(NewDeadline, bytes);
    check!(DeadlineSpan, bytes);
    check!(TaskFlags, bytes);
    check!(Retention, bytes);
    check!(CreatedGiven, bytes);
    check!(OwnerOutcomeCreated, bytes);
    check!(AccountHolder, bytes);
    check!(AskSignIn, bytes);
    check!(AskSignOut, bytes);
    check!(Ask, bytes);
}

fn records_8(bytes: &[u8]) {
    check!(OwnerProposalChoice, bytes);
    check!(OwnerEscalationChoice, bytes);
    check!(OwnerOutcomeMarkedSeen, bytes);
    check!(OwnerOutcomeNoteEdited, bytes);
    check!(OwnerOutcomeServiceMade, bytes);
    check!(OwnerOutcomeAdopted, bytes);
    check!(OwnerOutcomeStopped, bytes);
    check!(OwnerOutcomeCancelled, bytes);
    check!(OwnerOutcomeReleased, bytes);
    check!(OwnerOutcomePersonTaken, bytes);
    check!(OwnerOutcomePersonHandedBack, bytes);
    check!(OwnerOutcomePersonAnswered, bytes);
    check!(OwnerOutcomeMoved, bytes);
    check!(OwnerOutcomeProposalDecided, bytes);
    check!(OwnerOutcomeSaid, bytes);
    check!(OwnerOutcomeQuestionAnswered, bytes);
    check!(OwnerOutcomeAmended, bytes);
    check!(OwnerOutcomeProposed, bytes);
    check!(OwnerOutcomeWithdrawn, bytes);
    check!(OwnerOutcomeRolesSet, bytes);
    check!(OwnerOutcomePolicyChanged, bytes);
    check!(OwnerOutcomeAccountSignInStarted, bytes);
    check!(OwnerOutcomeAccountSignedOut, bytes);
    check!(OwnerOutcomeRenamed, bytes);
    check!(OwnerOutcomeProjectMade, bytes);
    check!(NameRule, bytes);
    check!(MalformedName, bytes);
    check!(OwnerOutcomePoolSet, bytes);
    check!(OwnerOutcomeEscalationDecided, bytes);
    check!(OwnerOutcome, bytes);
    check!(MadeTasks, bytes);
    check!(MadeService, bytes);
    check!(MadePersonResult, bytes);
    check!(MadeNote, bytes);
    check!(Made, bytes);
    check!(OutcomeOwner, bytes);
    check!(OutcomeMade, bytes);
    check!(OutcomeProposed, bytes);
    check!(OutcomeDecided, bytes);
}

fn records_9(bytes: &[u8]) {
    check!(OutcomeDecidedFirst, bytes);
    check!(Outcome, bytes);
    check!(Grant, bytes);
    check!(LackingBudget, bytes);
    check!(LackingGrant, bytes);
    check!(LackingGrantKind, bytes);
    check!(LackingAgain, bytes);
    check!(LackingRequired, bytes);
    check!(LackingFailed, bytes);
    check!(LackingUnguarded, bytes);
    check!(LackingTaskKind, bytes);
    check!(LackingDepth, bytes);
    check!(LackingDeadline, bytes);
    check!(LackingTasks, bytes);
    check!(Lacking, bytes);
    check!(AdmissionLimit, bytes);
    check!(LimitAdmission, bytes);
    check!(LimitLiveTasks, bytes);
    check!(LimitInbox, bytes);
    check!(LimitWords, bytes);
    check!(Limit, bytes);
    check!(CurrentHoldDocument, bytes);
    check!(CurrentHoldReason, bytes);
    check!(CurrentHold, bytes);
    check!(MovedHold, bytes);
    check!(Moved, bytes);
    check!(RefusalRole, bytes);
    check!(RefusalTaken, bytes);
    check!(RefusalAuthority, bytes);
    check!(RefusalLimit, bytes);
    check!(RefusalMoved, bytes);
    check!(RefusalEnded, bytes);
    check!(Refusal, bytes);
    check!(Done, bytes);
    check!(Refused, bytes);
    check!(Final, bytes);
}

fn records_10(bytes: &[u8]) {
    check!(Cursor, bytes);
    check!(InboxReadGroup, bytes);
    check!(InboxGroup, bytes);
    check!(ReadTask, bytes);
    check!(ReadProposal, bytes);
    check!(ReadEscalation, bytes);
    check!(ReadQuestion, bytes);
    check!(ReadPersonTask, bytes);
    check!(ReadResult, bytes);
    check!(ReadEnded, bytes);
    check!(ReadHistory, bytes);
    check!(ReadActivity, bytes);
    check!(ReadInbox, bytes);
    check!(ReadSpend, bytes);
    check!(ReadProject, bytes);
    check!(ReadNotes, bytes);
    check!(Read, bytes);
    check!(ReadNone, bytes);
    check!(ReadAnswerNone, bytes);
    check!(ReadAnswer, bytes);
    check!(OwnerPeriodTimes, bytes);
    check!(OwnerFundingRecord, bytes);
    check!(OwnerSpendPage, bytes);
}

fn records_11(bytes: &[u8]) {
    check!(OwnerMessageProposalKind, bytes);
    check!(OwnerNoticeState, bytes);
    check!(OwnerNewsClass, bytes);
    check!(OwnerWordResultApproved, bytes);
    check!(OwnerWordResultVerdict, bytes);
    check!(OwnerWordResultChange, bytes);
    check!(OwnerWordResult, bytes);
    check!(OwnerMessageKindEscalation, bytes);
    check!(OwnerMessageKindProposal, bytes);
    check!(OwnerMessageKindPersonStalled, bytes);
    check!(OwnerMessageKindProposalDecision, bytes);
    check!(OwnerMessageKindEffectNotMade, bytes);
    check!(OwnerMessageKindAmendment, bytes);
    check!(OwnerMessageKindAnswer, bytes);
    check!(OwnerMessageKindNotice, bytes);
    check!(OwnerMessageKindTimer, bytes);
    check!(OwnerMessageKindNews, bytes);
    check!(OwnerMessageKindResult, bytes);
    check!(OwnerMessageKind, bytes);
    check!(OwnerWord, bytes);
    check!(OwnerRunWhyMessage, bytes);
    check!(OwnerRunWhyRetry, bytes);
    check!(OwnerRunWhy, bytes);
    check!(OwnerRunEndFailed, bytes);
    check!(OwnerRunEnd, bytes);
    check!(OwnerActivityWords, bytes);
    check!(OwnerActivityHeard, bytes);
    check!(OwnerActivityNotified, bytes);
    check!(OwnerActivityRunBegan, bytes);
    check!(OwnerActivityRunEnded, bytes);
    check!(OwnerActivityTurn, bytes);
    check!(OwnerActivityDecided, bytes);
    check!(OwnerActivityMilestone, bytes);
    check!(OwnerActivity, bytes);
    check!(OwnerActivityLine, bytes);
    check!(TurnName, bytes);
    check!(TurnCarried, bytes);
    check!(TurnTooLarge, bytes);
    check!(TurnBytes, bytes);
    check!(ArchivedTurn, bytes);
    check!(RunBeginningFresh, bytes);
}

fn records_12(bytes: &[u8]) {
    check!(RunBeginning, bytes);
    check!(RunEndingFailed, bytes);
    check!(RunEnding, bytes);
    check!(FieldRule, bytes);
    check!(VerdictChoice, bytes);
    check!(TaskContractDocumentApproval, bytes);
    check!(TaskContractDocumentReport, bytes);
    check!(TaskContractDocumentVerdict, bytes);
    check!(TaskContractDocumentChange, bytes);
    check!(TaskContractDocument, bytes);
    check!(OwnerTaskStatus, bytes);
    check!(OwnerTaskLinkLive, bytes);
    check!(OwnerTaskLinkRetained, bytes);
    check!(OwnerTaskLinkStored, bytes);
    check!(OwnerTaskLink, bytes);
    check!(ResultDetails, bytes);
    check!(ActKind, bytes);
    check!(CreateProcedure, bytes);
    check!(OwnerKindImplication, bytes);
    check!(OwnerAuthorityDocument, bytes);
    check!(BoundsCreate, bytes);
    check!(BoundsAmend, bytes);
    check!(Bounds, bytes);
    check!(BeyondProposed, bytes);
    check!(BeyondRefused, bytes);
    check!(Beyond, bytes);
    check!(AdmissionProposed, bytes);
    check!(AdmissionBounded, bytes);
    check!(AdmissionRefused, bytes);
    check!(Admission, bytes);
    check!(Offered, bytes);
    check!(ResultDocument, bytes);
    check!(ObjectKeyTask, bytes);
    check!(ObjectKeyProposal, bytes);
    check!(ObjectKeyEscalation, bytes);
    check!(ObjectKeyQuestion, bytes);
    check!(ObjectKeyPersonTask, bytes);
    check!(ObjectKeyResult, bytes);
}

fn records_13(bytes: &[u8]) {
    check!(ObjectKeyNote, bytes);
    check!(ObjectKey, bytes);
    check!(ActivityOwner, bytes);
    check!(ActivityWords, bytes);
    check!(ActivityRunBegan, bytes);
    check!(ActivityRunEnded, bytes);
    check!(ActivityTurn, bytes);
    check!(ActivityResult, bytes);
    check!(ActivityNews, bytes);
    check!(ActivityDecision, bytes);
    check!(Activity, bytes);
    check!(ActivityLine, bytes);
    check!(ActivityPage, bytes);
    check!(SpendLine, bytes);
    check!(Hold, bytes);
    check!(HistoryChangeMade, bytes);
    check!(HistoryChangeAmended, bytes);
    check!(HistoryChangeHeld, bytes);
    check!(HistoryChangeCancelled, bytes);
    check!(HistoryChangeMoved, bytes);
    check!(HistoryChange, bytes);
    check!(HistoryLine, bytes);
    check!(OwnerHistoryChange, bytes);
    check!(ParameterDocumentNumber, bytes);
    check!(ParameterDocumentBytes, bytes);
    check!(ParameterDocumentResource, bytes);
    check!(ParameterDocument, bytes);
    check!(DocumentSpecification, bytes);
    check!(SpecificationDocument, bytes);
    check!(ProposedTask, bytes);
    check!(OwnerRetainedAmendment, bytes);
    check!(OwnerRetainedActionBatch, bytes);
    check!(OwnerRetainedActionEffect, bytes);
    check!(OwnerRetainedActionProcedureEffect, bytes);
    check!(OwnerRetainedActionAmend, bytes);
    check!(OwnerRetainedActionWiden, bytes);
    check!(OwnerRetainedActionRelease, bytes);
}

fn records_14(bytes: &[u8]) {
    check!(OwnerRetainedAction, bytes);
    check!(OwnerHistoryHolderTask, bytes);
    check!(OwnerHistoryHolderPerson, bytes);
    check!(OwnerHistoryHolderPolicy, bytes);
    check!(OwnerHistoryHolder, bytes);
    check!(OwnerProposalHistoryStatePending, bytes);
    check!(OwnerProposalHistoryStateAccepted, bytes);
    check!(OwnerProposalHistoryStateRejected, bytes);
    check!(OwnerProposalHistoryState, bytes);
    check!(OwnerProposalHistory, bytes);
    check!(OwnerAmendmentState, bytes);
    check!(OwnerHistoryAmendment, bytes);
    check!(OwnerHistoryLine, bytes);
    check!(OwnerMilestonePhaseActive, bytes);
    check!(OwnerMilestonePhaseClosing, bytes);
    check!(OwnerMilestonePhaseHeld, bytes);
    check!(OwnerMilestonePhaseEnded, bytes);
    check!(OwnerMilestonePhase, bytes);
    check!(OwnerMilestoneLine, bytes);
    check!(HistoryRowLine, bytes);
    check!(HistoryRowChange, bytes);
    check!(HistoryRowMilestone, bytes);
    check!(HistoryRow, bytes);
    check!(HistoryPage, bytes);
    check!(RuntimeActive, bytes);
    check!(RuntimeWaiting, bytes);
    check!(RuntimeRunning, bytes);
    check!(RuntimeWaitingForWords, bytes);
    check!(RuntimeBackingOff, bytes);
    check!(RuntimeProcedure, bytes);
    check!(RuntimePerson, bytes);
    check!(RuntimeClosing, bytes);
    check!(RuntimeHeld, bytes);
    check!(Runtime, bytes);
    check!(Count, bytes);
    check!(InboxEntry, bytes);
    check!(InboxPage, bytes);
}

fn records_15(bytes: &[u8]) {
    check!(OwnerPhaseCounts, bytes);
    check!(Budget, bytes);
    check!(Pool, bytes);
    check!(TaskSpend, bytes);
    check!(SpendPeriod, bytes);
    check!(SpendPage, bytes);
    check!(NoteAuthorParty, bytes);
    check!(NoteAuthorTask, bytes);
    check!(NoteAuthor, bytes);
    check!(Note, bytes);
    check!(NotePage, bytes);
    check!(ObjectPage, bytes);
    check!(OwnerEndedDelegate, bytes);
    check!(OwnerEndedPage, bytes);
    check!(OwnerInboxObjectQuestion, bytes);
    check!(OwnerInboxObjectProposal, bytes);
    check!(OwnerInboxObjectEscalation, bytes);
    check!(OwnerInboxObjectTurn, bytes);
    check!(OwnerInboxObjectResult, bytes);
    check!(OwnerInboxObject, bytes);
    check!(OwnerInboxCursorNumber, bytes);
    check!(OwnerInboxCursorInbox, bytes);
    check!(OwnerInboxCursor, bytes);
    check!(OwnerInboxSelection, bytes);
    check!(OwnerInboxEntry, bytes);
    check!(OwnerInboxPage, bytes);
}

fn decoders(bytes: &[u8]) {
    documents::decoders(bytes);
    records_1(bytes);
    records_2(bytes);
    records_3(bytes);
    records_4(bytes);
    records_5(bytes);
    records_6(bytes);
    records_7(bytes);
    records_8(bytes);
    records_9(bytes);
    records_10(bytes);
    records_11(bytes);
    records_12(bytes);
    records_13(bytes);
    records_14(bytes);
    records_15(bytes);
}

fn arbitrary(rng: &mut Rng) {
    let mut buffer = [0; 256];
    for _ in 0..2000 {
        let length = usize::try_from(rng.below(257)).expect("bounded length");
        for byte in buffer.get_mut(..length).expect("within buffer") {
            *byte = u8::try_from(rng.next_u64() & 0xff).expect("low byte");
        }
        decoders(buffer.get(..length).expect("within buffer"));
    }
}

fn mutation_seed(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    name.ends_with("_full.bin")
        && [
            "record_keyed_",
            "record_owned_",
            "record_refused_",
            "enum_ask_",
            "record_rename_",
            "record_make_project_",
            "record_owner_outcome_renamed_",
            "record_owner_outcome_project_made_",
            "record_malformed_name_",
            "enum_owner_outcome_renamed_",
            "enum_owner_outcome_project_made_",
            "enum_refusal_name_taken_",
            "enum_malformed_name_",
            "enum_name_rule_",
            "enum_admission_limit_projects_",
            "enum_act_kind_rename_",
            "enum_final_",
            "enum_fact_",
            "record_hold_reason_",
            "enum_read_",
            "enum_read_answer_",
            "enum_read_value_",
            "enum_delivery_value_",
            "record_chips_",
            "record_owner_inbox_",
            "record_stream_text_",
            "record_owner_spend_page_",
            "record_activity_page_",
            "record_history_page_",
            "record_owner_inbox_page_",
            "record_owner_historical_",
            "enum_owner_goal_holding_",
            "record_owner_person_proposal_goal_",
            "record_owner_project_",
            "record_deployment_",
            "record_owner_account_",
            "record_owner_deployment_",
            "record_open_",
            "record_authorize_",
            "enum_authorize_",
            "record_hello_",
            "enum_stream_event_",
            "record_run_state_",
            "record_task_",
        ]
        .iter()
        .any(|prefix| name.starts_with(prefix))
}

fn mutated(rng: &mut Rng) {
    let root = skein_scratch::paths::workspace().join("crates/jig-client-protocol-wire/golden");
    let mut paths: Vec<_> =
        fs::read_dir(root).expect("golden directory").map(|entry| entry.expect("golden entry").path()).collect();
    paths.sort();
    for path in paths {
        if !mutation_seed(&path) {
            continue;
        }
        let golden = fs::read(path).expect("golden bytes");
        decoders(&golden);
        for _ in 0..1 {
            let mut bytes = golden.clone();
            if !bytes.is_empty() {
                let index =
                    usize::try_from(rng.below(u64::try_from(bytes.len()).expect("length fits"))).expect("index fits");
                *bytes.get_mut(index).expect("chosen byte") = u8::try_from(rng.next_u64() & 0xff).expect("low byte");
            }
            decoders(&bytes);
        }
    }
}

#[test]
fn client_wire_decoders_never_panic_and_reencode_the_same() {
    let mut rng = Rng::new(0xc11e_0030);
    arbitrary(&mut rng);
    mutated(&mut rng);
}
