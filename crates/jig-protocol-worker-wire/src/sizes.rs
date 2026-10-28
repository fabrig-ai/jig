//! Channel bounds from the owners' numbers (protocol/channel.md, 9 and 11).
use crate::{Kind, v9};

/// Bounds supplied by the domains and the application's protocol layer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Sizes {
    /// Whole program sources per worker, independent of run slots.
    pub program_sources: u32,
    /// Bytes in one live piece (protocol/channel.md, 6.4 and 11).
    pub piece: u32,
    pub charter: u32,
    /// The whole resumed window, including each turn's four-byte length.
    pub transcript: u32,
    pub turn: u32,
    pub message: u32,
    pub sender: u32,
    pub call_name: u32,
    pub tool_name: u32,
    pub call_input: u32,
    pub answer: u32,
    pub result: u32,
    pub detail: u32,
    pub grant: u32,
    pub name: u32,
    pub secret: u32,
    pub model_name: u32,
    pub items: u32,
    pub left: u32,
    pub saved: u32,
    pub evidence: u32,
    pub resource: u32,
    pub slots: u32,
    pub calls: u32,
    pub turns: u32,
    pub inbox: u32,
    pub answered: u32,
    pub models: u32,
    pub workstreams: u32,
    pub accounts: u32,
    pub credentials: u32,
    /// Bytes reserved for facts, including their frame headers.
    pub facts: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Cap {
    pub bytes: u32,
    pub frames: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct WorkerCap {
    pub cap: Cap,
    pub facts: u32,
}

/// Map field and count bounds without inspecting any payload or sum.
#[must_use]
pub fn limits(sizes: &Sizes) -> Option<v9::Limits> {
    let limits = v9::Limits {
        input_call_id: v9::CEILINGS.input_call_id,
        input_call_tool: v9::CEILINGS.input_call_tool,
        piece_bytes: sizes.piece,
        program_source_name: 32,
        sources_sources: sizes.program_sources,
        check_name: 32,
        sign_out_name: 32,
        source_failed_detail: sizes.detail,
        login_otherwise_method: 32,
        login_name: 32,
        logins_logins: sizes.program_sources,
        signed_out_name: 32,
        credential_name: sizes.name,
        credential_secret: sizes.secret,
        model_bound_name: sizes.model_name,
        bounds_program_endpoints: v9::CEILINGS.bounds_program_endpoints,
        bounds_models: sizes.models,
        hello_workstreams: sizes.workstreams,
        hello_hosting: sizes.slots,
        turn_refused: sizes.inbox,
        turn_body: sizes.turn,
        call_name: sizes.call_name,
        call_tool: sizes.tool_name,
        call_input: sizes.call_input,
        work_left_some: sizes.left,
        work_saved_some: sizes.saved,
        ended_outcome: sizes.result,
        resource_resource_some: sizes.resource,
        failed_detail: sizes.detail,
        invalid_start_why: sizes.detail,
        // A provider's code and reason are smith's text, within the detail's bound.
        account_refused_code: sizes.detail.min(v9::CEILINGS.account_refused_code),
        account_refused_reason: sizes.detail.min(v9::CEILINGS.account_refused_reason),
        answer_refused: sizes.inbox,
        message_sender: sizes.sender,
        message_words: sizes.message,
        workspace_items: sizes.items,
        host_answer_body: sizes.answer,
        delivery_answer_evidence: sizes.evidence,
        answered_call_name: sizes.call_name,
        answered_call_tool: sizes.tool_name,
        credential_value_bytes: sizes.grant,
        application_value_secret: sizes.grant,
        program_value_name: v9::CEILINGS.program_source_name,
        assignment_charter: sizes.charter,
        assignment_transcript: sizes.transcript,
        assignment_answered: sizes.answered,
        assignment_messages: sizes.inbox,
        assignment_grants: sizes.accounts.checked_add(sizes.credentials)?,
        inbound_sender: sizes.sender,
        inbound_words: sizes.message,
        relay_text_text: sizes.answer,
        relayed_name: sizes.call_name,
    };
    v9::check_limits(&limits).ok()?;
    Some(limits)
}

#[must_use]
pub fn largest_body(sizes: &Sizes, kind: Kind) -> Option<u32> {
    body(&limits(sizes)?, kind)
}

fn body(limits: &v9::Limits, kind: Kind) -> Option<u32> {
    let bytes = match kind {
        Kind::Hello => v9::Hello::worst_case_bytes(limits),
        Kind::Turn => v9::Turn::worst_case_bytes(limits),
        Kind::Call => v9::Call::worst_case_bytes(limits),
        Kind::Answer => v9::Answer::worst_case_bytes(limits),
        Kind::Fact => v9::Fact::worst_case_bytes(limits),
        Kind::Unauthorized => v9::Unauthorized::worst_case_bytes(limits),
        Kind::Limited => v9::Limited::worst_case_bytes(limits),
        Kind::AccountRefused => v9::AccountRefused::worst_case_bytes(limits),
        Kind::Assignment => v9::Assignment::worst_case_bytes(limits),
        Kind::Message => v9::Inbound::worst_case_bytes(limits),
        Kind::Relayed => v9::Relayed::worst_case_bytes(limits),
        Kind::TurnKept => v9::TurnKept::worst_case_bytes(limits),
        Kind::TurnBusy => v9::TurnBusy::worst_case_bytes(limits),
        Kind::Acknowledge => v9::Acknowledge::worst_case_bytes(limits),
        Kind::Cancel => v9::Cancel::worst_case_bytes(limits),
        Kind::Park => v9::Park::worst_case_bytes(limits),
        Kind::Sources => v9::Sources::worst_case_bytes(limits),
        Kind::Check => v9::Check::worst_case_bytes(limits),
        Kind::SignOut => v9::SignOut::worst_case_bytes(limits),
        Kind::Logins => v9::Logins::worst_case_bytes(limits),
        Kind::SignedOut => v9::SignedOut::worst_case_bytes(limits),
        Kind::Grant => v9::Granted::worst_case_bytes(limits),
    }?;
    u32::try_from(bytes).ok()
}

fn frame(limits: &v9::Limits, kind: Kind) -> Option<u32> {
    body(limits, kind)?.checked_add(8)
}

#[must_use]
pub fn engine_cap(sizes: &Sizes) -> Option<Cap> {
    let limits = limits(sizes)?;
    let grants = sizes.accounts.checked_add(sizes.credentials)?;
    let mut bytes = frame(&limits, Kind::Assignment)?;
    bytes = bytes.checked_add(sizes.inbox.checked_mul(frame(&limits, Kind::Message)?)?)?;
    bytes = bytes.checked_add(sizes.calls.checked_mul(frame(&limits, Kind::Relayed)?)?)?;
    bytes = bytes.checked_add(sizes.turns.checked_mul(frame(&limits, Kind::TurnKept)?)?)?;
    bytes = bytes.checked_add(frame(&limits, Kind::Acknowledge)?)?;
    bytes = bytes.checked_add(frame(&limits, Kind::Cancel)?)?;
    bytes = bytes.checked_add(frame(&limits, Kind::Park)?)?;
    bytes = bytes.checked_add(grants.checked_mul(frame(&limits, Kind::Grant)?)?)?;
    let frames = 4_u32.checked_add(sizes.inbox)?.checked_add(sizes.calls)?.checked_add(sizes.turns)?;
    let frames = frames.checked_add(grants)?.checked_mul(sizes.slots)?;
    let sources = frame(&limits, Kind::Sources)?;
    let commands =
        sizes.program_sources.checked_mul(frame(&limits, Kind::Check)?.checked_add(frame(&limits, Kind::SignOut)?)?)?;
    Some(Cap {
        bytes: bytes.checked_mul(sizes.slots)?.checked_add(sources)?.checked_add(commands)?,
        frames: frames.checked_add(1)?.checked_add(sizes.program_sources.checked_mul(2)?)?,
    })
}

#[must_use]
pub fn worker_cap(sizes: &Sizes) -> Option<WorkerCap> {
    let limits = limits(sizes)?;
    let mut run = sizes.turns.checked_mul(frame(&limits, Kind::Turn)?)?;
    run = run.checked_add(frame(&limits, Kind::Answer)?)?;
    run = run.checked_add(sizes.calls.checked_mul(frame(&limits, Kind::Call)?)?)?;
    let notices = frame(&limits, Kind::Unauthorized)?
        .checked_add(frame(&limits, Kind::Limited)?)?
        .checked_add(frame(&limits, Kind::AccountRefused)?)?;
    let mut bytes = frame(&limits, Kind::Hello)?.checked_add(sizes.slots.checked_mul(run)?)?;
    bytes = bytes.checked_add(sizes.accounts.checked_mul(notices)?)?;
    // LongDone is the shortest fact: task + attempt + value tag + header.
    let fact_frames = sizes.facts / 25;
    let run_frames = sizes.turns.checked_add(1)?.checked_add(sizes.calls)?;
    let mut frames = 1_u32.checked_add(sizes.slots.checked_mul(run_frames)?)?;
    frames = frames.checked_add(sizes.accounts.checked_mul(3)?)?.checked_add(fact_frames)?;
    bytes = bytes
        .checked_add(frame(&limits, Kind::Logins)?)?
        .checked_add(sizes.program_sources.checked_mul(frame(&limits, Kind::SignedOut)?)?)?;
    frames = frames.checked_add(1)?.checked_add(sizes.program_sources)?;
    Some(WorkerCap { cap: Cap { bytes: bytes.checked_add(sizes.facts)?, frames }, facts: sizes.facts })
}
