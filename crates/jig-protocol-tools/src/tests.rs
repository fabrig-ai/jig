use crate::{Decoded, Problem, Why};
use alloc::boxed::Box;
use jig_domain_core as core;
mod fixtures;
mod llm;

const PROPOSED_NUMBER: u64 = 9;
const TASK_HOLDER: u64 = 3;
const PERSON_HOLDER: u64 = 4;
const FIRST_MADE: u64 = 7;
const SECOND_MADE: u64 = 8;

#[test]
fn a_cancel_call_decodes_to_its_named_action() {
    match crate::decode(b"cancel", fixtures::CANCEL, &fixtures::LIMITS) {
        Decoded::Action(core::NamedAction::Control {
            target: 3,
            control: jig_domain_tasks::Control::Cancel { .. },
        }) => {}
        other @ (Decoded::Action(_) | Decoded::Delegate(_) | Decoded::Problem(_) | Decoded::NotMine) => {
            panic!("unexpected decoded input {other:?}")
        }
    }
}

#[test]
fn a_release_call_decodes_to_its_named_action() {
    match crate::decode(b"release", fixtures::RELEASE, &fixtures::LIMITS) {
        Decoded::Action(core::NamedAction::Release { target: 3, revision: 1, again: Some(false) }) => {}
        other @ (Decoded::Action(_) | Decoded::Delegate(_) | Decoded::Problem(_) | Decoded::NotMine) => {
            panic!("unexpected decoded input {other:?}")
        }
    }
}

#[test]
fn a_introduce_call_decodes_to_its_named_action() {
    match crate::decode(b"introduce", fixtures::INTRODUCE, &fixtures::LIMITS) {
        Decoded::Action(core::NamedAction::Introduce { left: 3, right: 4 }) => {}
        other @ (Decoded::Action(_) | Decoded::Delegate(_) | Decoded::Problem(_) | Decoded::NotMine) => {
            panic!("unexpected decoded input {other:?}")
        }
    }
}

#[test]
fn a_decide_call_decodes_to_its_named_action() {
    match crate::decode(b"decide", fixtures::DECIDE, &fixtures::LIMITS) {
        Decoded::Action(core::NamedAction::DecideProposal { proposer: 3, proposal: 2, .. }) => {}
        other @ (Decoded::Action(_) | Decoded::Delegate(_) | Decoded::Problem(_) | Decoded::NotMine) => {
            panic!("unexpected decoded input {other:?}")
        }
    }
}

#[test]
fn a_withdraw_call_decodes_to_its_named_action() {
    match crate::decode(b"withdraw", fixtures::WITHDRAW, &fixtures::LIMITS) {
        Decoded::Action(core::NamedAction::WithdrawProposal { proposal: 2 }) => {}
        other @ (Decoded::Action(_) | Decoded::Delegate(_) | Decoded::Problem(_) | Decoded::NotMine) => {
            panic!("unexpected decoded input {other:?}")
        }
    }
}

#[test]
fn a_subscribe_call_decodes_to_its_named_action() {
    match crate::decode(b"subscribe", fixtures::SUBSCRIBE, &fixtures::LIMITS) {
        Decoded::Action(core::NamedAction::Subscribe { .. }) => {}
        other @ (Decoded::Action(_) | Decoded::Delegate(_) | Decoded::Problem(_) | Decoded::NotMine) => {
            panic!("unexpected decoded input {other:?}")
        }
    }
}

#[test]
fn a_unsubscribe_call_decodes_to_its_named_action() {
    match crate::decode(b"unsubscribe", fixtures::UNSUBSCRIBE, &fixtures::LIMITS) {
        Decoded::Action(core::NamedAction::Unsubscribe { subscription: 1 }) => {}
        other @ (Decoded::Action(_) | Decoded::Delegate(_) | Decoded::Problem(_) | Decoded::NotMine) => {
            panic!("unexpected decoded input {other:?}")
        }
    }
}

#[test]
fn a_amend_call_decodes_to_its_named_action() {
    match crate::decode(b"amend", fixtures::AMEND, &fixtures::LIMITS) {
        Decoded::Action(core::NamedAction::Amend { target: 3, .. }) => {}
        other @ (Decoded::Action(_) | Decoded::Delegate(_) | Decoded::Problem(_) | Decoded::NotMine) => {
            panic!("unexpected decoded input {other:?}")
        }
    }
}

#[test]
fn a_decide_escalation_call_decodes_to_its_named_action() {
    match crate::decode(b"decide", fixtures::DECIDE_ESCALATION, &fixtures::LIMITS) {
        Decoded::Action(core::NamedAction::DecideEscalation { task: 3, revision: 1, .. }) => {}
        other @ (Decoded::Action(_) | Decoded::Delegate(_) | Decoded::Problem(_) | Decoded::NotMine) => {
            panic!("unexpected decoded input {other:?}")
        }
    }
}

#[test]
fn a_propose_call_decodes_to_its_named_action() {
    match crate::decode(b"propose", fixtures::PROPOSE, &fixtures::LIMITS) {
        Decoded::Action(core::NamedAction::ProposeBatch { .. }) => {}
        other @ (Decoded::Action(_) | Decoded::Delegate(_) | Decoded::Problem(_) | Decoded::NotMine) => {
            panic!("unexpected decoded input {other:?}")
        }
    }
}

#[test]
fn a_note_call_decodes_to_its_named_action() {
    match crate::decode(b"note", fixtures::NOTE, &fixtures::LIMITS) {
        Decoded::Action(core::NamedAction::Note { .. }) => {}
        other @ (Decoded::Action(_) | Decoded::Delegate(_) | Decoded::Problem(_) | Decoded::NotMine) => {
            panic!("unexpected decoded input {other:?}")
        }
    }
}

#[test]
fn a_recall_call_decodes_to_its_named_action() {
    match crate::decode(b"recall", fixtures::RECALL, &fixtures::LIMITS) {
        Decoded::Action(core::NamedAction::Recall { .. }) => {}
        other @ (Decoded::Action(_) | Decoded::Delegate(_) | Decoded::Problem(_) | Decoded::NotMine) => {
            panic!("unexpected decoded input {other:?}")
        }
    }
}

#[test]
fn a_message_call_decodes_to_its_named_action() {
    match crate::decode(b"message", fixtures::MESSAGE, &fixtures::LIMITS) {
        Decoded::Action(core::NamedAction::Message { target, words, kind }) => {
            assert_eq!(target, 3);
            assert_eq!(words.as_ref(), b"is the build green?");
            assert_eq!(kind, jig_domain_tasks::MessageKind::Words);
        }
        other @ (Decoded::Action(_) | Decoded::Delegate(_) | Decoded::Problem(_) | Decoded::NotMine) => {
            panic!("message decoded as {other:?}")
        }
    }
}
#[test]
fn a_delegate_call_decodes_its_batch_in_order() {
    match crate::decode(b"delegate", fixtures::DELEGATE, &fixtures::LIMITS) {
        Decoded::Delegate(batch) => {
            assert_eq!(batch.len(), 2);
            assert_eq!(batch[0].spec.words.as_ref(), b"check");
            assert_eq!(batch[1].spec.words.as_ref(), b"report");
        }
        other @ (Decoded::Action(_) | Decoded::Problem(_) | Decoded::NotMine) => {
            panic!("delegate decoded as {other:?}")
        }
    }
}
#[test]
fn a_call_missing_a_required_field_names_it() {
    match crate::decode(b"message", br#"{"form":"words","words":"is the build green?"}"#, &fixtures::LIMITS) {
        Decoded::Problem(Problem { field: b"target", why: Why::Missing }) => {}
        other @ (Decoded::Action(_) | Decoded::Delegate(_) | Decoded::Problem(_) | Decoded::NotMine) => {
            panic!("unexpected decoded input {other:?}")
        }
    }
}
#[test]
fn a_call_with_the_wrong_field_type_names_it() {
    match crate::decode(b"message", br#"{"target":"three","form":"words","words":"hi"}"#, &fixtures::LIMITS) {
        Decoded::Problem(Problem { field: b"target", why: Why::Type }) => {}
        other @ (Decoded::Action(_) | Decoded::Delegate(_) | Decoded::Problem(_) | Decoded::NotMine) => {
            panic!("unexpected decoded input {other:?}")
        }
    }
}
#[test]
fn a_call_that_is_not_json_is_malformed() {
    match crate::decode(b"message", b"not JSON", &fixtures::LIMITS) {
        Decoded::Problem(Problem { why: Why::Malformed, .. }) => {}
        other @ (Decoded::Action(_) | Decoded::Delegate(_) | Decoded::Problem(_) | Decoded::NotMine) => {
            panic!("unexpected decoded input {other:?}")
        }
    }
}
#[test]
fn an_input_past_its_bound_is_too_large() {
    let limits = crate::Limits { input_bytes: 1, ..fixtures::LIMITS };
    match crate::decode(b"message", fixtures::MESSAGE, &limits) {
        Decoded::Problem(Problem { why: Why::TooLarge, .. }) => {}
        other @ (Decoded::Action(_) | Decoded::Delegate(_) | Decoded::Problem(_) | Decoded::NotMine) => {
            panic!("unexpected decoded input {other:?}")
        }
    }
}
#[test]
fn another_applications_tool_is_not_mine() {
    match crate::decode(b"another_tool", b"not JSON", &fixtures::LIMITS) {
        Decoded::NotMine => {}
        other @ (Decoded::Action(_) | Decoded::Delegate(_) | Decoded::Problem(_)) => {
            panic!("unexpected decoded input {other:?}")
        }
    }
}
#[test]
fn a_batch_past_its_bound_names_the_batch() {
    let limits = crate::Limits { batch: 1, ..fixtures::LIMITS };
    match crate::decode(b"delegate", fixtures::DELEGATE, &limits) {
        Decoded::Problem(Problem { field: b"batch", why: Why::TooLarge }) => {}
        other @ (Decoded::Action(_) | Decoded::Delegate(_) | Decoded::Problem(_) | Decoded::NotMine) => {
            panic!("unexpected decoded input {other:?}")
        }
    }
}
#[test]
fn a_delegated_part_renders_its_task_numbers() {
    assert_eq!(
        crate::render(&core::CallPart::Delegated(Box::new([
            core::Made { task: 3, revision: 0, given: None },
            core::Made { task: 12, revision: 0, given: None }
        ]))),
        crate::Rendered {
            error: false,
            text: b"Delegated tasks: 3 at spec revision 0; 12 at spec revision 0".as_slice().into()
        }
    );
}
#[test]
fn a_denied_part_renders_as_an_error_naming_what_it_lacked() {
    let part = core::CallPart::ToolDenied {
        answer: jig_domain_authority::Answer::Refuse,
        findings: Box::new([jig_domain_authority::Finding::Tool]),
    };
    assert_eq!(
        crate::render(&part),
        crate::Rendered { error: true, text: b"Refused; missing tool authority".as_slice().into() }
    );
}
#[test]
fn an_unproposable_part_renders_as_an_error_saying_no_proposal_could_be_accepted() {
    let part = core::CallPart::ProposalRefused(jig_domain_tasks::Problem {
        task: Some(1),
        why: jig_domain_tasks::Refusal::Unproposable,
        blocked_by: None,
        capacity: None,
    });
    assert_eq!(
        crate::render(&part),
        crate::Rendered { error: true, text: b"Refused: no proposal could be accepted".as_slice().into() }
    );
}
#[test]
fn a_problem_renders_the_tools_name_and_missing_field() {
    assert_eq!(
        crate::problem(b"message", &Problem { field: b"target", why: Why::Missing }),
        crate::Rendered { error: true, text: b"message: the field target is missing".as_slice().into() }
    );
}
#[test]
fn a_call_with_duplicate_fields_is_malformed() {
    match crate::decode(b"release", br#"{"target":3,"target":4}"#, &fixtures::LIMITS) {
        Decoded::Problem(Problem { why: Why::Malformed, .. }) => {}
        other @ (Decoded::Action(_) | Decoded::Delegate(_) | Decoded::Problem(_) | Decoded::NotMine) => {
            panic!("unexpected decoded input {other:?}")
        }
    }
}
fn schema(tool: &[u8]) {
    let limits = crate::Limits { input_bytes: 65536, ..fixtures::LIMITS };
    let mut declared = None;
    for declaration in crate::declarations() {
        if declaration.name == tool {
            declared = Some(declaration);
        }
    }
    let declaration = declared.expect("the declared tool");
    let schema = crate::json::parse(declaration.schema, &limits).expect("a schema is JSON");
    assert!(!declaration.description.is_empty());
    let mut expected = None;
    for (name, fields) in fixtures::FIELDS {
        if *name == declaration.name {
            expected = Some(*fields);
        }
    }
    for field in expected.expect("every tool has decoder fields") {
        assert!(schema_names(&schema, field), "the schema names {field:?}");
    }
}

#[test]
fn the_delegate_schema_parses_and_names_its_decoders_fields() {
    schema(b"delegate");
}

#[test]
fn the_message_schema_parses_and_names_its_decoders_fields() {
    schema(b"message");
}

#[test]
fn the_cancel_schema_parses_and_names_its_decoders_fields() {
    schema(b"cancel");
}

#[test]
fn the_release_schema_parses_and_names_its_decoders_fields() {
    schema(b"release");
}

#[test]
fn the_introduce_schema_parses_and_names_its_decoders_fields() {
    schema(b"introduce");
}

#[test]
fn the_decide_schema_parses_and_names_its_decoders_fields() {
    schema(b"decide");
}

#[test]
fn the_withdraw_schema_parses_and_names_its_decoders_fields() {
    schema(b"withdraw");
}

#[test]
fn the_subscribe_schema_parses_and_names_its_decoders_fields() {
    schema(b"subscribe");
}

#[test]
fn the_unsubscribe_schema_parses_and_names_its_decoders_fields() {
    schema(b"unsubscribe");
}

#[test]
fn the_amend_schema_parses_and_names_its_decoders_fields() {
    schema(b"amend");
}

#[test]
fn the_propose_schema_parses_and_names_its_decoders_fields() {
    schema(b"propose");
}

#[test]
fn the_note_schema_parses_and_names_its_decoders_fields() {
    schema(b"note");
}

#[test]
fn the_recall_schema_parses_and_names_its_decoders_fields() {
    schema(b"recall");
}

fn schema_names(schema: &crate::json::Value, field: &[u8]) -> bool {
    if let Some(properties) = schema.field(b"properties").expect("schema object") {
        return properties.field(field).expect("properties object").is_some();
    }
    let choices = schema.required(b"oneOf").expect("a union schema").array().expect("schema alternatives");
    for choice in choices {
        let properties = choice.required(b"properties").expect("alternative properties");
        if properties.field(field).expect("properties object").is_some() {
            return true;
        }
    }
    false
}
#[test]
fn words_past_their_bound_name_the_field() {
    let limits = crate::Limits { words: 8, ..fixtures::LIMITS };
    match crate::decode(b"message", fixtures::MESSAGE, &limits) {
        Decoded::Problem(Problem { field: b"words", why: Why::TooLarge }) => {}
        other @ (Decoded::Action(_) | Decoded::Delegate(_) | Decoded::Problem(_) | Decoded::NotMine) => {
            panic!("unexpected decoded input {other:?}")
        }
    }
}
#[test]
fn nesting_past_its_bound_is_too_large() {
    let limits = crate::Limits { depth: 1, ..fixtures::LIMITS };
    match crate::decode(b"delegate", fixtures::DELEGATE, &limits) {
        Decoded::Problem(Problem { why: Why::TooLarge, .. }) => {}
        other @ (Decoded::Action(_) | Decoded::Delegate(_) | Decoded::Problem(_) | Decoded::NotMine) => {
            panic!("unexpected decoded input {other:?}")
        }
    }
}

#[test]
fn an_amend_call_names_the_spec_revision_it_saw() {
    let Decoded::Action(core::NamedAction::Amend { target, revision, .. }) =
        crate::decode(b"amend", fixtures::AMEND, &fixtures::LIMITS)
    else {
        panic!("decoded amendment")
    };
    assert_eq!((target, revision), (3, 1));
}

#[test]
fn an_amend_call_without_a_revision_is_answered_missing() {
    let Decoded::Problem(problem) =
        crate::decode(b"amend", br#"{"target":3,"amendment":{"reason":"check again"}}"#, &fixtures::LIMITS)
    else {
        panic!("missing revision")
    };
    assert_eq!(problem, Problem { field: b"revision", why: Why::Missing });
}

#[test]
fn a_revised_answer_names_the_spec_revision_made() {
    let answer = crate::render(&core::CallPart::Revised { task: 7, revision: 2 });
    assert_eq!(answer.text.as_ref(), b"Done: task 7 is at spec revision 2");
    assert!(!answer.error);
}

#[test]
fn a_moved_answer_names_the_hold_and_its_reason() {
    let answer = crate::render(&core::CallPart::Moved {
        task: 7,
        now: jig_domain_people::Moved::Hold { revision: 3, reason: jig_domain_people::HoldReason::Budget },
    });
    assert_eq!(answer.text.as_ref(), b"Moved: task 7 is held at hold revision 3 now, for its budget");
    assert!(answer.error);
}

#[test]
fn a_release_call_names_the_hold_revision_and_again() {
    let Decoded::Action(core::NamedAction::Release { target, revision, again }) =
        crate::decode(b"release", fixtures::RELEASE, &fixtures::LIMITS)
    else {
        panic!("release")
    };
    assert_eq!((target, revision, again), (fixtures::TARGET, 1, Some(false)));
}
#[test]
fn a_decide_call_names_an_escalation_by_its_hold_revision() {
    let Decoded::Action(core::NamedAction::DecideEscalation {
        task,
        revision,
        choice: core::EscalationChoice::Release,
    }) = crate::decode(b"decide", fixtures::DECIDE_ESCALATION, &fixtures::LIMITS)
    else {
        panic!("escalation")
    };
    assert_eq!((task, revision), (fixtures::TARGET, 1));
}
#[test]
fn a_decide_call_names_a_proposal_by_its_proposer_and_number() {
    let Decoded::Action(core::NamedAction::DecideProposal { proposer, proposal, choice: core::ProposalChoice::Accept }) =
        crate::decode(b"decide", fixtures::DECIDE, &fixtures::LIMITS)
    else {
        panic!("proposal")
    };
    assert_eq!((proposer, proposal), (fixtures::TARGET, fixtures::PROPOSAL));
}
#[test]
fn decide_escalation_is_no_longer_a_declared_tool() {
    for tool in crate::declarations() {
        assert_ne!(tool.name, b"decide_escalation");
    }
    match crate::decode(b"decide_escalation", b"{}", &fixtures::LIMITS) {
        Decoded::NotMine => {}
        other @ (Decoded::Action(_) | Decoded::Delegate(_) | Decoded::Problem(_)) => {
            panic!("removed tool decoded: {other:?}")
        }
    }
}

mod words;

#[test]
fn a_delegate_call_names_label_flags_and_as_allowed() {
    let Decoded::Delegate(batch) = crate::decode(b"delegate", fixtures::DELEGATE_NAMED, &fixtures::LIMITS) else {
        panic!("delegate batch");
    };
    assert_eq!(batch[0].label, 7);
    assert_eq!(batch[0].tracked, Some(4));
    assert_eq!(
        batch[0].flags,
        jig_domain_tasks::Flags {
            ended_by: jig_domain_tasks::EndedBy::Result,
            conversation: true,
            listed: true,
            retention: jig_domain_tasks::Retention::ForgottenOnceRead,
        }
    );
    assert_eq!(batch[0].budget, jig_domain_people::Budget::AsAllowed { cap: Some(40) });
}

#[test]
fn a_proposed_answer_names_the_holder_it_waits_for() {
    use jig_domain_tasks::{ProposalHolder, ProposalKind};
    for (holder, expected) in [
        (ProposalHolder::Task(TASK_HOLDER), b"Proposed 9, to task 3".as_slice()),
        (ProposalHolder::Person(PERSON_HOLDER), b"Proposed 9, to person 4"),
        (
            ProposalHolder::Policy { project: 1, kind: ProposalKind::Amend },
            b"Proposed 9, to the project's role for amendments",
        ),
    ] {
        assert_eq!(
            crate::render(&core::CallPart::Proposed { proposal: PROPOSED_NUMBER, holder }).text.as_ref(),
            expected,
            "{holder:?}"
        );
    }
}

#[test]
fn a_delegated_answer_names_each_spec_revision_and_what_was_given() {
    let part = core::CallPart::Delegated(Box::new([
        core::Made { task: FIRST_MADE, revision: 0, given: None },
        core::Made { task: SECOND_MADE, revision: 0, given: Some(40) },
    ]));
    assert_eq!(
        crate::render(&part).text.as_ref(),
        b"Delegated tasks: 7 at spec revision 0; 8 at spec revision 0, given 40"
    );
}

#[test]
fn a_propose_call_amends_naming_the_spec_revision() {
    let input = br#"{"action":"amend","target":3,"revision":7,"amendment":{"spec":{"words":"new"},"reason":"evidence"},"reason":"please"}"#;
    let Decoded::Action(core::NamedAction::Propose {
        action: jig_domain_tasks::ProposalAction::Amend { task, revision, amendment },
        reason,
        as_holder,
    }) = crate::decode(b"propose", input, &fixtures::LIMITS)
    else {
        panic!("amend proposal")
    };
    assert_eq!((task, revision, as_holder), (fixtures::TARGET, 7, false));
    assert_eq!(amendment.spec.expect("changed spec").words.as_ref(), b"new");
    assert_eq!(reason.as_ref(), b"please");
}
#[test]
fn a_propose_call_widens_its_own_authority() {
    let input = br#"{"action":"widen","authority":{"tools":0,"grants":[],"delegation":{"kinds":[],"tasks":0,"depth":0},"budget":{"spend":40},"notes":0},"reason":"please"}"#;
    let Decoded::Action(core::NamedAction::Propose {
        action: jig_domain_tasks::ProposalAction::Widen { task, authority },
        reason,
        as_holder,
    }) = crate::decode(b"propose", input, &fixtures::LIMITS)
    else {
        panic!("widen proposal")
    };
    assert_eq!((task, authority.budget.spend, as_holder), (0, 40, false));
    assert_eq!(reason.as_ref(), b"please");
}
#[test]
fn a_delegate_batch_with_two_as_allowed_budgets_is_not_well_formed() {
    let Decoded::Problem(problem) = crate::decode(b"delegate", fixtures::TWO_AS_ALLOWED, &fixtures::LIMITS) else {
        panic!("malformed batch")
    };
    assert_eq!(problem, Problem { field: b"budget", why: Why::Malformed });
}

const CANCEL_GRACE: &[u8] = br#"{"target":3,"reason":"wrap up","grace":5000000000}"#;
const TRACK_PRIORITY: &[u8] = br#"{"target":3,"amendment":{"tracked":{"priority":7},"reason":"urgent"}}"#;
const SPEC_NO_REVISION: &[u8] = br#"{"target":3,"amendment":{"spec":{"words":"new"},"reason":"evidence"}}"#;

#[test]
fn a_cancel_call_gives_a_grace() {
    let Decoded::Action(core::NamedAction::Control { target, control }) =
        crate::decode(b"cancel", CANCEL_GRACE, &fixtures::LIMITS)
    else {
        panic!("decoded cancellation")
    };
    assert_eq!(target, fixtures::TARGET);
    assert_eq!(
        control,
        jig_domain_tasks::Control::Cancel {
            reason: b"wrap up".as_slice().into(),
            grace: Some(skein_lib::Duration::from_secs(5)),
            message: None,
        }
    );
}
#[test]
fn a_cancel_call_without_a_grace_decodes_as_before() {
    let Decoded::Action(core::NamedAction::Control { target, control }) =
        crate::decode(b"cancel", fixtures::CANCEL, &fixtures::LIMITS)
    else {
        panic!("decoded cancellation")
    };
    assert_eq!(target, fixtures::TARGET);
    assert_eq!(
        control,
        jig_domain_tasks::Control::Cancel { reason: b"no longer needed".as_slice().into(), grace: None, message: None }
    );
}
#[test]
fn an_amend_of_a_priority_alone_needs_no_revision() {
    let Decoded::Action(core::NamedAction::Amend { target, revision, amendment }) =
        crate::decode(b"amend", TRACK_PRIORITY, &fixtures::LIMITS)
    else {
        panic!("decoded priority amendment")
    };
    assert_eq!((target, revision), (fixtures::TARGET, 0));
    assert_eq!(amendment.tracked, Some(jig_domain_tasks::Tracking::Tracked { priority: 7 }));
}
#[test]
fn an_amend_changing_the_spec_without_a_revision_is_answered_missing() {
    let Decoded::Problem(problem) = crate::decode(b"amend", SPEC_NO_REVISION, &fixtures::LIMITS) else {
        panic!("missing revision")
    };
    assert_eq!(problem, Problem { field: b"revision", why: Why::Missing });
}

const DECIDED_OUTCOMES: &[(jig_domain_tasks::ProposalOutcome, &[u8])] = &[
    (jig_domain_tasks::ProposalOutcome::Accepted, b"Already decided: proposal 9 was accepted"),
    (jig_domain_tasks::ProposalOutcome::Rejected, b"Already decided: proposal 9 was rejected"),
    (jig_domain_tasks::ProposalOutcome::Withdrawn, b"Already decided: proposal 9 was withdrawn"),
    (jig_domain_tasks::ProposalOutcome::Stale, b"Already decided: proposal 9 was stale"),
];
#[test]
fn a_decided_proposal_answer_names_its_outcome() {
    for &(outcome, expected) in DECIDED_OUTCOMES {
        let answer = crate::render(&core::CallPart::ProposalDecided { proposal: PROPOSED_NUMBER, outcome });
        assert_eq!(answer.text.as_ref(), expected, "{outcome:?}");
        assert!(!answer.error, "{outcome:?}");
    }
}

#[test]
fn a_release_proposal_names_its_task_hold_and_revision() {
    let decoded = crate::decode(
        b"propose",
        br#"{"action":"release","task":7,"hold":31,"revision":2,"reason":"resume"}"#,
        &fixtures::LIMITS,
    );
    match decoded {
        Decoded::Action(core::NamedAction::Propose {
            action: jig_domain_tasks::ProposalAction::ReleasePlaced { task: 7, hold: 31, revision: 2 },
            ..
        }) => {}
        other @ (Decoded::Action(_) | Decoded::Delegate(_) | Decoded::Problem(_) | Decoded::NotMine) => {
            panic!("unexpected release proposal {other:?}")
        }
    }
    for malformed in [
        br#"{"action":"release","task":7,"revision":2,"reason":"resume"}"#.as_slice(),
        br#"{"action":"release","task":7,"hold":31,"reason":"resume"}"#.as_slice(),
    ] {
        match crate::decode(b"propose", malformed, &fixtures::LIMITS) {
            Decoded::Problem(_) => {}
            other @ (Decoded::Action(_) | Decoded::Delegate(_) | Decoded::NotMine) => {
                panic!("malformed release was admitted {other:?}")
            }
        }
    }
}

#[test]
fn a_hold_owner_refusal_names_the_proposal_revision_and_owner() {
    for (owner, expected) in [
        (jig_domain_people::HoldOwner::Party { person: 9 }, b"task 7 is held by hold 31 (revision 2), placed by person 9; only they may release it: propose the release with the hold's revision.".as_slice()),
        (jig_domain_people::HoldOwner::Task { task: 3 }, b"task 7 is held by hold 31 (revision 2), placed by task 3; only they may release it: propose the release with the hold's revision.".as_slice()),
    ] {
        let rendered = crate::render(&core::CallPart::HoldOwner { task: 7, hold: 31, revision: 2, owner });
        assert!(rendered.error);
        assert_eq!(rendered.text.as_ref(), expected);
    }
}
