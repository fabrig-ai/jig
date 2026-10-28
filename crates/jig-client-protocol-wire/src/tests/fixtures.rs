//! Small codec limits and shared client contract records.
mod delivery_values;
mod read_values;
mod stream;
mod task_values;
use crate::*;
use alloc::boxed::Box;
pub use delivery_values::delivery_values;
pub use read_values::read_values;
use skein_lib::{List, Overflow, Writer};
pub use stream::*;
pub use task_values::task;

pub fn limits() -> Limits {
    limits_with_activity_bytes(CEILINGS.activity_page_rows)
}

pub fn limits_with_activity_bytes(bound: u32) -> Limits {
    let mut limits = CEILINGS;
    limits.keyed_request = 32;
    limits.owner_spend_page_rows = 2;
    limits.activity_page_rows = bound;
    check_limits(&limits).expect("client test limits below ceilings");
    limits
}

pub fn bytes(n: u32) -> Box<[u8]> {
    skein_lib::bytes::zeroed(usize::try_from(n).expect("small bound"))
}

pub fn ceiling_text(n: u32) -> Box<[u8]> {
    let mut words = bytes(n);
    words.fill(b'x');
    words
}

pub fn message() -> Message {
    let limits = limits();
    Message::new(&limits, MessageParts { project: 17, task: 19, words: ceiling_text(limits.message_words) })
        .expect("bounded message")
}

pub fn refused_moved() -> Refused {
    let limits = limits();
    let how = EndingTaskFields::new(&limits, EndingTaskFieldsParts { value: 2 }).expect("ending index");
    let reason = HoldReasonDependencyFields::new(&limits, HoldReasonDependencyFieldsParts { task: 31, how: 1 })
        .expect("dependency index");
    let words = DocumentText::new(&limits, DocumentTextParts { bytes: Box::from(b"failed".as_slice()), cut: true })
        .expect("bounded failure words");
    let failed = OwnerChipEndingFailedFields::new(&limits, OwnerChipEndingFailedFieldsParts { value: words })
        .expect("complete failure");
    let mut rows = List::with_capacity(3);
    rows.push(Fact::HoldReason(
        HoldReasonRow::new(&limits, HoldReasonRowParts { parent: None, value: HoldReasonFields::Dependency(reason) })
            .expect("root fact"),
    ))
    .expect("three rows");
    rows.push(Fact::Ending(
        EndingRow::new(&limits, EndingRowParts { parent: Some(0), value: EndingFields::Task(how) })
            .expect("ending fact"),
    ))
    .expect("three rows");
    rows.push(Fact::OwnerChipEnding(
        OwnerChipEndingRow::new(
            &limits,
            OwnerChipEndingRowParts { parent: Some(1), value: OwnerChipEndingFields::Failed(failed) },
        )
        .expect("failure fact"),
    ))
    .expect("three rows");
    let rows = piece_items(&rows, Fact::measure, Fact::encode);
    let reason = HoldReason::new(&limits, HoldReasonParts { rows }).expect("flattened reason");
    let reason = CurrentHoldDocument::new(&limits, CurrentHoldDocumentParts { value: reason }).expect("current hold");
    let what =
        MovedHold::new(&limits, MovedHoldParts { reason: Some(CurrentHold::Document(reason)) }).expect("moved hold");
    let now = Revision::new(&limits, RevisionParts { value: u64::MAX }).expect("whole revision");
    let moved = RefusalMoved::new(&limits, RefusalMovedParts { now, what: Moved::Hold(what) }).expect("refusal facts");
    Refused::new(&limits, RefusedParts { at: None, refusal: Refusal::Moved(moved) }).expect("complete refusal")
}

pub fn encoded<T>(value: &T, measure: fn(&T) -> u32, encode: fn(&T, &mut Writer) -> Result<(), Overflow>) -> Box<[u8]> {
    let mut writer = Writer::new(usize::try_from(measure(value)).expect("measured record"));
    encode(value, &mut writer).expect("measured room");
    writer.finish()
}

pub fn message_bytes() -> Box<[u8]> {
    encoded(&message(), Message::measure, Message::encode)
}

pub fn owned() -> Owned {
    let limits = limits();
    let family = Family::new(&limits, FamilyParts { number: u16::MAX }).expect("whole family number");
    Owned::new(&limits, OwnedParts { owner: Owner::Family(family), version: u32::MAX, record: message_bytes() })
        .expect("whole owner's version")
}

pub fn read_none(none: ReadNone) -> ReadAnswerNone {
    ReadAnswerNone::new(&limits(), ReadAnswerNoneParts { value: none }).expect("wire read outcome")
}

pub fn funding_rows(count: u32) -> List<OwnerFundingRecord> {
    let limits = limits();
    let period = OwnerPeriodTimes::new(&limits, OwnerPeriodTimesParts { began: u64::MAX, next: None })
        .expect("optional next period");
    let numbers =
        OwnerNumbers::new(&limits, OwnerNumbersParts { budget: u64::MAX, spent: 11, spent_below: 17, reserved: 23 })
            .expect("owner funding numbers");
    let source =
        FunderPeriod::new(&limits, FunderPeriodParts { project: 17, period: u64::MAX }).expect("whole period identity");
    let row = OwnerFundingRecord::new(
        &limits,
        OwnerFundingRecordParts {
            period: Some(period),
            funder: Funder::Period(source),
            parent: None,
            numbers,
            made: u32::MAX,
            closed: true,
        },
    )
    .expect("whole funding row");
    let mut rows = List::with_capacity(count);
    for _ in 0..count {
        rows.push(row.clone()).expect("reserved funding rows");
    }
    rows
}

pub fn piece_items<T>(
    rows: &List<T>,
    measure: fn(&T) -> u32,
    encode: fn(&T, &mut Writer) -> Result<(), Overflow>,
) -> Box<[u8]> {
    let mut length = 0_u32;
    for row in rows {
        length = length.checked_add(measure(row)).expect("small fixture row");
        length = length.checked_add(4).expect("small fixture piece");
    }
    let mut out = Writer::new(usize::try_from(length).expect("fixture length fits"));
    for row in rows {
        let bytes = encoded(row, measure, encode);
        skein_codec::piece::write(&mut out, &bytes).expect("measured piece room");
    }
    out.finish()
}

pub fn activity_line(turn: Box<[u8]>) -> ActivityLine {
    let limits = limits();
    let name = TurnName::new(&limits, TurnNameParts { attempt: u64::MAX, number: u32::MAX })
        .expect("whole committed turn name");
    let event = ActivityTurn::new(&limits, ActivityTurnParts { name, turn }).expect("opaque turn bytes");
    ActivityLine::new(&limits, ActivityLineParts { position: u64::MAX, at: u64::MAX, event: Activity::Turn(event) })
        .expect("whole activity line")
}

pub fn activity_rows(count: u32, width: u32) -> List<ActivityLine> {
    let row = activity_line(bytes(width));
    let mut rows = List::with_capacity(count);
    for _ in 0..count {
        rows.push(row.clone()).expect("reserved activity rows");
    }
    rows
}

pub fn activity_page(limits: &Limits, rows: Box<[u8]>) -> ActivityPage {
    ActivityPage::new(limits, ActivityPageParts { cut: true, rows, before: None, after: None })
        .expect("page within its piece bound")
}

pub fn encoded_activity_page(limits: &Limits, rows: &List<ActivityLine>) -> Box<[u8]> {
    let rows = piece_items(rows, ActivityLine::measure, ActivityLine::encode);
    encoded(&activity_page(limits, rows), ActivityPage::measure, ActivityPage::encode)
}

fn next_activity_row(limits: &Limits, reader: &mut skein_lib::Reader<'_>) -> Result<Option<ActivityLine>, Problem> {
    let bytes = match skein_codec::piece::split(reader) {
        Ok(bytes) => bytes,
        Err(reason) => return Err(Problem { path: Path::ActivityPageRows, reason }),
    };
    let Some(bytes) = bytes else { return Ok(None) };
    let row = ActivityLine::decode(limits, &mut skein_lib::Reader::new(&bytes))?;
    Ok(Some(row))
}

pub fn decode_activity_page(limits: &Limits, bytes: &[u8], row_limit: u32) -> Result<List<ActivityLine>, Problem> {
    let page = ActivityPage::decode(limits, &mut skein_lib::Reader::new(bytes))?;
    let mut reader = skein_lib::Reader::new(page.rows());
    let mut rows = List::with_capacity(row_limit);
    while let Some(row) = next_activity_row(limits, &mut reader)? {
        if rows.push(row).is_err() {
            return Err(Problem { path: Path::ActivityPageRows, reason: skein_codec::Reason::Bound });
        }
    }
    Ok(rows)
}

pub fn malformed_activity_pieces() -> [Box<[u8]>; 3] {
    let rows = activity_rows(1, 8);
    let valid = piece_items(&rows, ActivityLine::measure, ActivityLine::encode);
    let mut out = Writer::new(valid.len().checked_add(1).expect("small trailing fragment"));
    out.put(&valid).expect("room for valid row");
    out.put(&[255]).expect("room for trailing fragment");
    [Box::from([0, 0, 0]), Box::from([0, 0, 0, 5, 1, 2]), out.finish()]
}

// Shared task and run fixtures retain supplied values independently of wire order.
pub fn document_limits() -> Limits {
    let mut limits = CEILINGS;
    limits.document_text_bytes = 4;
    limits.me_projects = 512;
    check_limits(&limits).expect("document fixtures below ceilings");
    limits
}

pub fn repeated<T: Clone + core::fmt::Debug>(value: T, count: u32) -> List<T> {
    let mut values = List::with_capacity(count);
    for _ in 0..count {
        values.push(value.clone()).expect("reserved fixture list");
    }
    values
}

pub fn document_text(limits: &Limits) -> DocumentText {
    DocumentText::new(limits, DocumentTextParts { bytes: ceiling_text(limits.document_text_bytes), cut: true })
        .expect("bounded document_text")
}

pub fn owner_numbers(limits: &Limits) -> OwnerNumbers {
    OwnerNumbers::new(limits, OwnerNumbersParts { budget: u64::MAX, spent: 11, spent_below: 17, reserved: 23 })
        .expect("bounded owner_numbers")
}

pub fn funder_period(limits: &Limits) -> FunderPeriod {
    FunderPeriod::new(limits, FunderPeriodParts { project: u32::MAX, period: u64::MAX }).expect("bounded funder_period")
}

pub fn owner_budget(limits: &Limits) -> OwnerBudget {
    OwnerBudget::new(
        limits,
        OwnerBudgetParts {
            numbers: owner_numbers(limits),
            left: u64::MAX,
            funder: Funder::Period(funder_period(limits)),
            deadline: Some(u64::MAX),
        },
    )
    .expect("bounded owner_budget")
}

pub fn pool_ledger(limits: &Limits) -> PoolLedger {
    PoolLedger::new(
        limits,
        PoolLedgerParts {
            source: Funder::Period(funder_period(limits)),
            parent: Some(Funder::Task(funder_task(limits))),
            numbers: owner_numbers(limits),
            left: u64::MAX,
            closed: true,
            began: Some(u64::MAX),
            next: None,
        },
    )
    .expect("bounded pool_ledger")
}

pub fn funder_task(limits: &Limits) -> FunderTask {
    FunderTask::new(limits, FunderTaskParts { value: u64::MAX }).expect("bounded funder_task")
}

pub fn held_role_policy(limits: &Limits) -> HeldRolePolicy {
    HeldRolePolicy::new(limits, HeldRolePolicyParts { role: u32::MAX }).expect("bounded held_role_policy")
}

pub fn project_role(limits: &Limits) -> MeProject {
    MeProject::new(
        limits,
        MeProjectParts {
            models: List::with_capacity(0),
            templates: Box::from([]),
            creating: None,
            project: u32::MAX,
            name: document_text(limits),
            role: HeldRole::Policy(held_role_policy(limits)),
            pool: Some(pool_ledger(limits)),
        },
    )
    .expect("bounded project_role")
}

pub fn frame_party(limits: &Limits) -> MeParty {
    MeParty::new(
        limits,
        MePartyParts {
            number: u64::MAX,
            kind: PartyKind::Service,
            login: document_text(limits),
            name: document_text(limits),
        },
    )
    .expect("bounded frame_party")
}

pub fn needs(limits: &Limits) -> Needs {
    Needs::new(
        limits,
        NeedsParts { questions: u32::MAX, proposals: u32::MAX, escalations: u32::MAX, person_tasks: u32::MAX },
    )
    .expect("bounded needs")
}

pub fn count(limits: &Limits) -> Count {
    Count::new(limits, CountParts { value: u32::MAX, limited: true }).expect("bounded count")
}

pub fn run_state(limits: &Limits) -> RunState {
    RunState::new(
        limits,
        RunStateParts { task: u64::MAX, attempt: Some(u64::MAX), committed_turn: u32::MAX, writing: None, call: None },
    )
    .expect("bounded run_state")
}

pub fn revision(limits: &Limits) -> Revision {
    Revision::new(limits, RevisionParts { value: u64::MAX }).expect("bounded revision")
}

pub fn wake_rule_batch(limits: &Limits) -> WakeRuleBatch {
    WakeRuleBatch::new(limits, WakeRuleBatchParts { count: u32::MAX, age: skein_lib::Duration::from_nanos(u64::MAX) })
        .expect("bounded wake_rule_batch")
}

pub fn owner_wake_policy(limits: &Limits) -> OwnerWakePolicy {
    OwnerWakePolicy::new(
        limits,
        OwnerWakePolicyParts {
            words: WakeRule::Batch(wake_rule_batch(limits)),
            notices: WakeRule::Immediate,
            news: WakeRule::Never,
            results: ResultsWake::LastOrFailure,
            questions: true,
            answers: false,
            timers: true,
        },
    )
    .expect("bounded owner_wake_policy")
}

pub fn active_run(limits: &Limits) -> ActiveRun {
    ActiveRun::new(
        limits,
        ActiveRunParts {
            stage: RunStage::Idle,
            phase_since: u64::MAX,
            attempt: None,
            placement: Some(u64::MAX),
            retry: None,
            observation: None,
        },
    )
    .expect("bounded active_run")
}

pub fn executor_agent(limits: &Limits) -> ExecutorAgent {
    ExecutorAgent::new(limits, ExecutorAgentParts { charter: u32::MAX }).expect("bounded executor_agent")
}

pub fn chip_phase_owner_fields(limits: &Limits) -> ChipPhaseOwnerFields {
    ChipPhaseOwnerFields::new(limits, ChipPhaseOwnerFieldsParts { phase: OwnerChipPhase::Ended, details: 2 })
        .expect("bounded chip_phase_owner_fields")
}

pub fn chip_phase_active_fields(limits: &Limits) -> ChipPhaseActiveFields {
    ChipPhaseActiveFields::new(limits, ChipPhaseActiveFieldsParts { value: active_run(limits) })
        .expect("bounded chip_phase_active_fields")
}

pub fn chip_phase_waiting_fields(limits: &Limits) -> ChipPhaseWaitingFields {
    ChipPhaseWaitingFields::new(limits, ChipPhaseWaitingFieldsParts { dependencies: repeated(u64::MAX, 2), hold: None })
        .expect("bounded chip_phase_waiting_fields")
}

pub fn chip_phase_idle_fields(limits: &Limits) -> ChipPhaseIdleFields {
    ChipPhaseIdleFields::new(limits, ChipPhaseIdleFieldsParts { wake: wake_policy(limits) })
        .expect("bounded chip_phase_idle_fields")
}

pub fn wake_policy(limits: &Limits) -> WakePolicy {
    WakePolicy::new(
        limits,
        WakePolicyParts { words: true, results: false, topics: repeated(u16::MAX, 2), subscriptions: Box::from([]) },
    )
    .expect("bounded wake_policy")
}

pub fn chip_phase_running_fields(limits: &Limits) -> ChipPhaseRunningFields {
    ChipPhaseRunningFields::new(
        limits,
        ChipPhaseRunningFieldsParts { attempt: u64::MAX, since: u64::MAX, tool: Some(ceiling_text(4)) },
    )
    .expect("bounded chip_phase_running_fields")
}

pub fn chip_phase_waiting_for_words_fields(limits: &Limits) -> ChipPhaseWaitingForWordsFields {
    ChipPhaseWaitingForWordsFields::new(
        limits,
        ChipPhaseWaitingForWordsFieldsParts { attempt: u64::MAX, since: u64::MAX },
    )
    .expect("bounded chip_phase_waiting_for_words_fields")
}

pub fn chip_phase_backing_off_fields(limits: &Limits) -> ChipPhaseBackingOffFields {
    ChipPhaseBackingOffFields::new(
        limits,
        ChipPhaseBackingOffFieldsParts {
            at: u64::MAX,
            attempt: u64::MAX,
            tries: u32::MAX,
            class: FailureClass::Stall,
            reason: ceiling_text(4),
        },
    )
    .expect("bounded chip_phase_backing_off_fields")
}

pub fn chip_phase_procedure_fields(limits: &Limits) -> ChipPhaseProcedureFields {
    ChipPhaseProcedureFields::new(limits, ChipPhaseProcedureFieldsParts { panel: Part::Panel(part_panel(limits)) })
        .expect("bounded chip_phase_procedure_fields")
}

pub fn part_panel(limits: &Limits) -> PartPanel {
    PartPanel::new(limits, PartPanelParts { connector: u16::MAX, task: u64::MAX }).expect("bounded part_panel")
}

pub fn chip_phase_person_fields(limits: &Limits) -> ChipPhasePersonFields {
    ChipPhasePersonFields::new(
        limits,
        ChipPhasePersonFieldsParts {
            addressed: Recipient::Party(recipient_party(limits)),
            taken: Some(taken_by(limits)),
            since: None,
        },
    )
    .expect("bounded chip_phase_person_fields")
}

pub fn recipient_party(limits: &Limits) -> RecipientParty {
    RecipientParty::new(limits, RecipientPartyParts { value: u64::MAX }).expect("bounded recipient_party")
}

pub fn taken_by(limits: &Limits) -> TakenBy {
    TakenBy::new(limits, TakenByParts { number: u64::MAX, name: None }).expect("bounded taken_by")
}

pub fn chip_phase_closing_fields(limits: &Limits) -> ChipPhaseClosingFields {
    ChipPhaseClosingFields::new(limits, ChipPhaseClosingFieldsParts { value: 2 })
        .expect("bounded chip_phase_closing_fields")
}

pub fn chip_phase_held_fields(limits: &Limits) -> ChipPhaseHeldFields {
    ChipPhaseHeldFields::new(
        limits,
        ChipPhaseHeldFieldsParts { reason: 2, since: u64::MAX, revision: revision(limits), was: None },
    )
    .expect("bounded chip_phase_held_fields")
}

pub fn chip_phase_ended_fields(limits: &Limits) -> ChipPhaseEndedFields {
    ChipPhaseEndedFields::new(limits, ChipPhaseEndedFieldsParts { how: 2, at: u64::MAX })
        .expect("bounded chip_phase_ended_fields")
}

pub fn owner_executor_agent(limits: &Limits) -> OwnerExecutorAgent {
    OwnerExecutorAgent::new(limits, OwnerExecutorAgentParts { charter: u32::MAX })
        .expect("bounded owner_executor_agent")
}

pub fn owner_chip_details_fields(limits: &Limits) -> OwnerChipDetailsFields {
    OwnerChipDetailsFields::new(
        limits,
        OwnerChipDetailsFieldsParts {
            dependencies: List::with_capacity(0),
            project: u32::MAX,
            executor: OwnerExecutor::Agent(owner_executor_agent(limits)),
            phase_since: u64::MAX,
            run_since: None,
            attempt: Some(u64::MAX),
            retry: None,
            wake: owner_wake_policy(limits),
            person_wait: None,
            hold: None,
            budget: owner_budget(limits),
            activity: u64::MAX,
            resources: Box::from([]),
            resources_cut: true,
            resources_observed: false,
            procedure: None,
            live_delegates: u32::MAX,
            closing_connectors: Some(List::with_capacity(0)),
            ending: None,
            ended_at: None,
            observation: None,
        },
    )
    .expect("bounded owner_chip_details_fields")
}

pub fn closing_fields(limits: &Limits) -> ClosingFields {
    ClosingFields::new(
        limits,
        ClosingFieldsParts {
            delegates: u32::MAX,
            connectors: Some(repeated(u16::MAX, 2)),
            stage: Some(ClosingStage::Settled),
            ending: None,
        },
    )
    .expect("bounded closing_fields")
}

pub fn hold_reason_budget_fields(limits: &Limits) -> HoldReasonBudgetFields {
    HoldReasonBudgetFields::new(limits, HoldReasonBudgetFieldsParts { spent: u64::MAX, limit: u64::MAX })
        .expect("bounded hold_reason_budget_fields")
}

pub fn frame_at_ceilings(limits: &Limits) -> Me {
    Me::new(
        limits,
        MeParts {
            providers: Box::new([]),
            providers_cut: false,
            key_bytes: 256,
            redirect_bytes: 4096,
            words_bytes: 65_536,
            amendment_bytes: 65_536,
            party: frame_party(limits),
            projects: piece_items(&repeated(project_role(limits), 2), MeProject::measure, MeProject::encode),
            needs: needs(limits),
            needs_total: u32::MAX,
            updates: count(limits),
            results_seen: u64::MAX,
            cut: true,
            retention: skein_lib::Duration::from_nanos(u64::MAX),
            deployment_role: Some(HeldRole::Owner),
        },
    )
    .expect("frame at configured ceilings")
}

pub fn chip_phases(limits: &Limits) -> [ChipPhaseFields; 13] {
    [
        ChipPhaseFields::Owner(chip_phase_owner_fields(limits)),
        ChipPhaseFields::Active(chip_phase_active_fields(limits)),
        ChipPhaseFields::Waiting(chip_phase_waiting_fields(limits)),
        ChipPhaseFields::Idle(chip_phase_idle_fields(limits)),
        ChipPhaseFields::Starting,
        ChipPhaseFields::Running(chip_phase_running_fields(limits)),
        ChipPhaseFields::WaitingForWords(chip_phase_waiting_for_words_fields(limits)),
        ChipPhaseFields::BackingOff(chip_phase_backing_off_fields(limits)),
        ChipPhaseFields::Procedure(chip_phase_procedure_fields(limits)),
        ChipPhaseFields::Person(chip_phase_person_fields(limits)),
        ChipPhaseFields::Closing(chip_phase_closing_fields(limits)),
        ChipPhaseFields::Held(chip_phase_held_fields(limits)),
        ChipPhaseFields::Ended(chip_phase_ended_fields(limits)),
    ]
}

fn phase_fact(limits: &Limits, phase: &ChipPhaseFields) -> Option<Fact> {
    match phase {
        ChipPhaseFields::Owner(_) => Some(Fact::OwnerChipDetails(Box::new(
            OwnerChipDetailsRow::new(
                limits,
                OwnerChipDetailsRowParts { parent: Some(1), value: owner_chip_details_fields(limits) },
            )
            .expect("whole owner details"),
        ))),
        ChipPhaseFields::Closing(_) => Some(Fact::Closing(
            ClosingRow::new(limits, ClosingRowParts { parent: Some(1), value: closing_fields(limits) })
                .expect("whole closing"),
        )),
        ChipPhaseFields::Held(_) => Some(Fact::HoldReason(
            HoldReasonRow::new(
                limits,
                HoldReasonRowParts {
                    parent: Some(1),
                    value: HoldReasonFields::Budget(hold_reason_budget_fields(limits)),
                },
            )
            .expect("whole hold"),
        )),
        ChipPhaseFields::Ended(_) => Some(Fact::Ending(
            EndingRow::new(limits, EndingRowParts { parent: Some(1), value: EndingFields::Done })
                .expect("whole ending"),
        )),
        ChipPhaseFields::Active(_)
        | ChipPhaseFields::Waiting(_)
        | ChipPhaseFields::Idle(_)
        | ChipPhaseFields::Starting
        | ChipPhaseFields::Running(_)
        | ChipPhaseFields::WaitingForWords(_)
        | ChipPhaseFields::BackingOff(_)
        | ChipPhaseFields::Procedure(_)
        | ChipPhaseFields::Person(_) => None,
    }
}

fn compact_phase(phase: &ChipPhaseFields) -> Phase {
    match phase {
        ChipPhaseFields::Owner(_) => Phase::Ended,
        ChipPhaseFields::Active(_) | ChipPhaseFields::Procedure(_) | ChipPhaseFields::Person(_) => Phase::Active,
        ChipPhaseFields::Waiting(_) => Phase::Waiting,
        ChipPhaseFields::Idle(_) => Phase::Idle,
        ChipPhaseFields::Starting => Phase::Due,
        ChipPhaseFields::Running(_) | ChipPhaseFields::WaitingForWords(_) => Phase::Running,
        ChipPhaseFields::BackingOff(_) => Phase::BackingOff,
        ChipPhaseFields::Closing(_) => Phase::Closing,
        ChipPhaseFields::Held(_) => Phase::Held,
        ChipPhaseFields::Ended(_) => Phase::Done,
    }
}

pub fn chip_facts(limits: &Limits, phase: ChipPhaseFields) -> List<Fact> {
    let mut rows = List::with_capacity(3);
    let fields = ChipFields::new(
        limits,
        ChipFieldsParts {
            task: u64::MAX,
            title: document_text(limits),
            executor: Executor::Agent(executor_agent(limits)),
            phase: compact_phase(&phase),
            details: 1,
        },
    )
    .expect("whole chip header");
    rows.push(Fact::Chip(ChipRow::new(limits, ChipRowParts { parent: None, value: fields }).expect("chip root")))
        .expect("reserved root");
    let extra = phase_fact(limits, &phase);
    rows.push(Fact::ChipPhase(
        ChipPhaseRow::new(limits, ChipPhaseRowParts { parent: Some(0), value: phase }).expect("child phase"),
    ))
    .expect("reserved phase");
    if let Some(extra) = extra {
        rows.push(extra).expect("reserved details");
    }
    rows
}

pub fn chip(limits: &Limits, rows: &List<Fact>) -> Chip {
    let rows = piece_items(rows, Fact::measure, Fact::encode);
    Chip::new(limits, ChipParts { rows }).expect("whole chip piece")
}

pub fn decode_chip_facts(limits: &Limits, bytes: &[u8]) -> List<Fact> {
    let chip = Chip::decode(limits, &mut skein_lib::Reader::new(bytes)).expect("whole generated chip");
    let mut reader = skein_lib::Reader::new(chip.rows());
    let mut rows = List::with_capacity(3);
    while let Some(bytes) = skein_codec::piece::split(&mut reader).expect("exact chip pieces") {
        let fact = Fact::decode(limits, &mut skein_lib::Reader::new(&bytes)).expect("whole generated fact");
        rows.push(fact).expect("bounded fact rows");
    }
    rows
}

pub fn not_served() -> NotServed {
    let lim = limits();
    let mut served = List::with_capacity(1);
    served
        .push(Served::new(&lim, ServedParts { owner: Owner::Jig, first: 19, last: 19 }).expect("range"))
        .expect("range slot");
    NotServed::new(&lim, NotServedParts { owner: Owner::Jig, served }).expect("version refusal")
}
mod operations;
pub use operations::merged_targets;

mod compound;
pub use compound::{COMPOUND_PROJECT, compound_operations};

// The previous 111-byte minimum plus an empty name: four length bytes and its cut flag.
pub const OWNER_PROJECT_MIN_BYTES: u32 = 116;
