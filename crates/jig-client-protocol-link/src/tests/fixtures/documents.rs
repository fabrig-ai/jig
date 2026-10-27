use super::*;
use skein_lib::{List, Wall};
pub fn text(value: &[u8]) -> c::Text {
    c::Text::new(Box::from(value), 32).unwrap()
}
pub fn document_text(value: &[u8]) -> c::DocumentText {
    c::DocumentText { bytes: text(value), cut: false }
}

pub fn chip() -> c::Chip {
    c::Chip {
        task: 7,
        title: document_text(b""),
        executor: c::Executor::Agent { charter: 7 },
        phase: c::Phase::Waiting,
        details: Box::new(c::ChipPhase::Starting),
    }
}
pub fn chip_record() -> w::Chip {
    jig_client_test_records::chip_record()
}

pub fn chip_pieces(count: u32) -> Box<[u8]> {
    jig_client_test_records::chip_pieces(count)
}
pub fn frame_record() -> w::Me {
    jig_client_test_records::frame_record()
}
pub fn frame() -> c::Me {
    c::Me {
        providers: List::with_capacity(0),
        providers_cut: false,
        key_bytes: 256,
        redirect_bytes: 4096,
        party: c::MeParty {
            number: 7,
            kind: c::PartyKind::Person,
            login: c::DocumentText { bytes: text(b""), cut: false },
            name: c::DocumentText { bytes: text(b""), cut: false },
        },
        projects: List::with_capacity(0),
        needs: c::Needs { questions: 7, proposals: 7, escalations: 7, person_tasks: 7 },
        needs_total: 7,
        updates: c::Count { value: 7, limited: false },
        results_seen: 7,
        cut: false,
        retention: Duration::from_nanos(19),
        deployment_role: None,
    }
}
pub fn task_record() -> w::Task {
    jig_client_test_records::task_record()
}
pub fn task() -> c::Task {
    c::Task {
        dependency_facts: List::with_capacity(0),
        kept_account: None,
        run_wait: None,
        settings: Some(c::TaskSettings { task: 1, run: None, newest: 1 }),
        llm: None,
        label: 0,
        flags: c::TaskFlags {
            ended_by: c::EndedBy::Result,
            conversation: false,
            listed: false,
            retention: c::Retention::Kept,
        },
        last_activity: Wall::EPOCH,
        waits_on: c::WaitsOn::Nothing,
        chip: chip(),
        goal: None,
        priority: None,
        lineage_chips: List::with_capacity(0),
        inputs: List::with_capacity(0),
        project: 7,
        requester: c::OwnerRequester::Task(7),
        lineage: List::with_capacity(0),
        spec: c::SpecificationDocument {
            specification: c::DocumentSpecification {
                words: c::DocumentText { bytes: text(b""), cut: false },
                parameters: List::with_capacity(0),
                inputs: List::with_capacity(0),
            },
            cut: false,
        },
        spec_revision: c::Revision { value: 7 },
        person_revision: None,
        taken_by: None,
        instructions: None,
        wake: c::OwnerWakePolicy {
            markers: 0,
            words: c::WakeRule::Never,
            notices: c::WakeRule::Never,
            news: c::WakeRule::Never,
            results: c::ResultsWake::Never,
            questions: false,
            answers: false,
            timers: false,
        },
        subscriptions: c::Subscriptions { entries: List::with_capacity(0) },
        result_contract: c::TaskContractDocument::Approval {
            state: Box::from([]),
            part: c::OwnerPart {
                connector: 7,
                kind: c::OwnerPartKind::Resource,
                key: c::OwnerPartKey::Path(List::with_capacity(0)),
            },
        },
        dependencies: List::with_capacity(0),
        references: List::with_capacity(0),
        cut: false,
        resources: List::with_capacity(0),
        budget: c::OwnerBudget {
            numbers: c::OwnerNumbers { budget: 7, spent: 7, spent_below: 7, reserved: 7 },
            left: 7,
            funder: c::Funder::Task(7),
            deadline: None,
        },
        authority: c::OwnerAuthorityDocument {
            authority: c::OwnerAuthority {
                models: Some(c::AuthorityModels::All),
                tools: 7,
                grants: List::with_capacity(0),
                delegation: c::OwnerDelegation { kinds: List::with_capacity(0), tasks: 7, depth: 7 },
                spend: 7,
                deadline: None,
                notes: 7,
                note_resources: List::with_capacity(0),
            },
            cut: false,
        },
        hold: None,
        proposals: List::with_capacity(0),
        activity: 7,
        changed: Wall::from_nanos(7),
        acts: List::with_capacity(0),
        result: None,
        delegates: List::with_capacity(0),
        context: None,
    }
}
pub fn run_state_record() -> w::RunState {
    jig_client_test_records::run_state_record()
}
pub fn run_state() -> c::RunState {
    c::RunState { task: 7, attempt: None, committed_turn: 7, writing: None, call: None }
}
pub fn run_text_record() -> w::RunText {
    jig_client_test_records::run_text_record()
}
pub fn run_text() -> c::RunText {
    c::RunText {
        writing: None,
        attempt: 7,
        began: None,
        turn: c::ProvisionalTurnName { attempt: 7, number: 7, retry: 7 },
        from: 7,
        through: 7,
        text: text(b""),
        cut: false,
        call: None,
    }
}
pub fn current_call_record() -> w::CurrentCall {
    jig_client_test_records::current_call_record()
}
pub fn current_call() -> c::CurrentCall {
    c::CurrentCall {
        identity: c::CallIdentity { owner: Token::new(7), sequence: 7, retry: 7, id: text(b"") },
        tool: text(b""),
        input: text(b""),
        cut: false,
        began: None,
    }
}
pub fn deployment_record() -> w::Deployment {
    let lim = limits().wire;
    w::Deployment::new(
        &lim,
        w::DeploymentParts {
            hosts: Box::from([]),
            accounts: Box::from([]),
            capacity: List::with_capacity(0),
            proposals: 7,
            refusals: Box::from([]),
            parts: Box::from([]),
        },
    )
    .unwrap()
}
pub fn deployment() -> c::Deployment {
    c::Deployment {
        hosts: List::with_capacity(0),
        accounts: List::with_capacity(0),
        capacity: List::with_capacity(0),
        proposals: 7,
        refusals: List::with_capacity(0),
        parts: List::with_capacity(0),
    }
}
pub fn inbox_record() -> w::Inbox {
    let lim = limits().wire;
    w::Inbox::new(
        &lim,
        w::InboxParts {
            entries: Box::from([]),
            needs: w::Needs::new(&lim, w::NeedsParts { questions: 7, proposals: 7, escalations: 7, person_tasks: 7 })
                .unwrap(),
            updates: w::Count::new(&lim, w::CountParts { value: 7, limited: false }).unwrap(),
            results_seen: 7,
            replies_seen: List::with_capacity(0),
            more: false,
        },
    )
    .unwrap()
}
pub fn inbox() -> c::Inbox {
    c::Inbox {
        entries: List::with_capacity(0),
        needs: c::Needs { questions: 7, proposals: 7, escalations: 7, person_tasks: 7 },
        updates: c::Count { value: 7, limited: false },
        results_seen: 7,
        replies_seen: List::with_capacity(0),
        more: false,
    }
}
pub fn tasks_record() -> w::Tasks {
    jig_client_test_records::tasks_record()
}
pub fn tasks() -> c::Tasks {
    c::Tasks { rows: List::with_capacity(0), more: false }
}
pub fn tree_record() -> w::Tree {
    let lim = limits().wire;
    w::Tree::new(
        &lim,
        w::TreeParts {
            task: 7,
            nodes: Box::from([]),
            folded: w::PhaseCounts::new(
                &lim,
                w::PhaseCountsParts {
                    ended: 7,
                    waiting: 7,
                    running: 7,
                    held: 7,
                    closing: 7,
                    done: 7,
                    failed: 7,
                    cancelled: 7,
                },
            )
            .unwrap(),
            held_below: 7,
        },
    )
    .unwrap()
}
pub fn tree() -> c::Tree {
    c::Tree {
        task: 7,
        nodes: List::with_capacity(0),
        folded: c::PhaseCounts {
            ended: 7,
            waiting: 7,
            running: 7,
            held: 7,
            closing: 7,
            done: 7,
            failed: 7,
            cancelled: 7,
        },
        held_below: 7,
    }
}
pub fn task_with_lineage(count: u32) -> w::Task {
    let lim = limits().wire;
    let mut parts = task_record().into_parts();
    parts.lineage_chips = chip_pieces(count);
    w::Task::new(&lim, parts).unwrap()
}
pub fn text_record() -> w::RunText {
    let lim = limits().wire;
    w::RunText::new(
        &lim,
        w::RunTextParts {
            writing: None,
            attempt: 41,
            began: Some(u64::MAX),
            turn: w::ProvisionalTurnName::new(&lim, w::ProvisionalTurnNameParts { attempt: 41, number: 7, retry: 3 })
                .unwrap(),
            from: 5,
            through: 11,
            text: Box::from(b"offset".as_slice()),
            cut: true,
            call: None,
        },
    )
    .unwrap()
}
pub fn run_text_expected() -> c::RunText {
    c::RunText {
        writing: None,
        attempt: 41,
        began: Some(Wall::from_nanos(u64::MAX)),
        turn: c::ProvisionalTurnName { attempt: 41, number: 7, retry: 3 },
        from: 5,
        through: 11,
        text: text(b"offset"),
        cut: true,
        call: None,
    }
}
pub fn task_with_piece(bytes: &[u8]) -> w::Task {
    let lim = limits().wire;
    let mut parts = task_record().into_parts();
    parts.lineage_chips = Box::from(bytes);
    w::Task::new(&lim, parts).unwrap()
}
pub fn limits_for_document(bytes: u64) -> Limits {
    let mut client = limits().client;
    client.document_bytes = bytes;
    Limits::new(client, 4).unwrap()
}

pub fn task_at_row_bound() -> w::Task {
    task_with_lineage(2)
}
pub fn task_with_two_chips() -> c::Task {
    let mut value = task();
    value.lineage_chips = List::with_capacity(2);
    value.lineage_chips.push(chip()).unwrap();
    value.lineage_chips.push(chip()).unwrap();
    value
}
pub fn run_text_one_past_text_limit() -> w::RunText {
    let mut parts = text_record().into_parts();
    parts.text = Box::from([b'x'; 33]);
    w::RunText::new(&w::CEILINGS, parts).unwrap()
}

pub fn raw_live_call() -> w::LiveCall {
    let lim = limits().wire;
    let identity = w::CallIdentity::new(
        &lim,
        w::CallIdentityParts { owner: 17, sequence: 2, retry: 3, id: Box::from(b"provider".as_slice()) },
    )
    .unwrap();
    let value = w::CurrentCall::new(
        &lim,
        w::CurrentCallParts {
            identity,
            tool: Box::from(b"read".as_slice()),
            input: Box::from(b"x".as_slice()),
            cut: true,
            began: Some(0),
        },
    )
    .unwrap();
    w::LiveCall::Raw(w::LiveCallRaw::new(&lim, w::LiveCallRawParts { value }).unwrap())
}
pub fn raw_live_call_expected() -> c::LiveCall {
    c::LiveCall::Raw(c::CurrentCall {
        identity: c::CallIdentity { owner: Token::new(17), sequence: 2, retry: 3, id: text(b"provider") },
        tool: text(b"read"),
        input: text(b"x"),
        cut: true,
        began: Some(Wall::from_nanos(0)),
    })
}
pub fn decoded_live_call() -> w::LiveCall {
    let lim = limits().wire;
    let value = w::Call::new(
        &lim,
        w::CallParts {
            number: 11,
            tool: Box::from(b"read".as_slice()),
            input: Box::from(b"x".as_slice()),
            outcome: Some(Box::from(b"yes".as_slice())),
            failed: false,
            objects: List::with_capacity(0),
            parts: Box::from([]),
        },
    )
    .unwrap();
    w::LiveCall::Decoded(w::LiveCallDecoded::new(&lim, w::LiveCallDecodedParts { value }).unwrap())
}
pub fn decoded_live_call_expected() -> c::LiveCall {
    c::LiveCall::Decoded(c::Call {
        number: 11,
        tool: text(b"read"),
        input: text(b"x"),
        outcome: Some(text(b"yes")),
        failed: false,
        objects: List::with_capacity(0),
        parts: List::with_capacity(0),
    })
}

pub fn task_with_resource_path(count: u32) -> w::Task {
    let mut wire = limits().wire;
    wire.owner_part_key_path_value = count;
    let mut parts = task_record().into_parts();
    let w::TaskContractDocument::Approval(approval) = parts.result_contract else { panic!("approval fixture") };
    let mut approval = approval.into_parts();
    let mut part = approval.part.into_parts();
    let mut path = List::with_capacity(count);
    for _ in 0..count {
        path.push(Box::from(b"segment".as_slice())).unwrap();
    }
    part.key =
        w::OwnerPartKey::Path(w::OwnerPartKeyPath::new(&wire, w::OwnerPartKeyPathParts { value: path }).unwrap());
    approval.part = w::OwnerPart::new(&wire, part).unwrap();
    parts.result_contract =
        w::TaskContractDocument::Approval(w::TaskContractDocumentApproval::new(&wire, approval).unwrap());
    w::Task::new(&wire, parts).unwrap()
}

pub fn task_with_authority_grants(count: u32) -> (w::Task, c::Task) {
    let mut expected = task();
    expected.authority.authority.grants = List::with_capacity(count);
    for _ in 0..count {
        expected
            .authority
            .authority
            .grants
            .push(c::OwnerGrant {
                connector: u16::MAX,
                kind: u16::MAX,
                pattern: c::OwnerPattern {
                    segments: List::with_capacity(0),
                    last: c::OwnerPatternEnd::Open(Box::new([])),
                },
            })
            .unwrap();
    }
    let mut parts = task_record().into_parts();
    parts.authority = w::OwnerAuthorityDocument::new(
        &w::CEILINGS,
        w::OwnerAuthorityDocumentParts {
            cut: false,
            authority: translate::asks::wire_owner_authority(&expected.authority.authority, &w::CEILINGS).unwrap(),
        },
    )
    .unwrap();
    (w::Task::new(&w::CEILINGS, parts).unwrap(), expected)
}
