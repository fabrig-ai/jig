use super::fixtures::{self, SIZES_LARGE, SIZES_ONE_SLOT};
use crate::Kind;
use crate::sizes::{Cap, Sizes, WorkerCap, engine_cap, largest_body, limits, worker_cap};

#[test]
fn the_largest_turn_kept_body_is_twenty_bytes() {
    // Task u64 + attempt u64 + number u32.
    assert_eq!(largest_body(&SIZES_ONE_SLOT, Kind::TurnKept), Some(20));
}

#[test]
fn the_engine_cap_for_one_slot_is_the_hand_summed_bytes() {
    // Assignment342, message65, relayed58, kept20, ack16, cancel16, park16, grants2*73;
    // Nine frame headers add72:350+73+66+28+24+24+24+162=751.
    // Sources84 + two checks44 + two sign-outs44 add260 and five frames.
    assert_eq!(engine_cap(&SIZES_ONE_SLOT), Some(Cap { bytes: 1011, frames: 14 }));
}

#[test]
fn the_worker_cap_for_one_slot_is_the_hand_summed_bytes() {
    // Hello385 (125 + endpoint count 4 + 64 * 4) + turn71 + answer136 + call85 + unauthorized28 + limited29
    // + account refused60; seven headers56. Sources add Logins158 + two SignedOut66.
    // Mandatory1140 + reserve390 =1530; reserve fits fifteen shortest facts25.
    assert_eq!(worker_cap(&SIZES_ONE_SLOT), Some(WorkerCap { cap: Cap { bytes: 1530, frames: 25 }, facts: 390 }));
}

#[test]
fn the_worker_cap_reserves_the_facts_share_last() {
    let sizes = Sizes { facts: 75, ..SIZES_ONE_SLOT };
    // Mandatory frames1140 bytes + reserve75 =1215; three shortest facts25 each.
    assert_eq!(worker_cap(&sizes), Some(WorkerCap { cap: Cap { bytes: 1215, frames: 13 }, facts: 75 }));
}

#[test]
fn the_largest_turn_body_matches_a_measured_fullest_turn() {
    let turn = fixtures::fullest_turn();
    // Fixed28 + read9 + window1 + refusal list13 + body20 =71.
    assert_eq!(turn.measure(), 71);
    assert_eq!(largest_body(&SIZES_ONE_SLOT, Kind::Turn), Some(71));
}

#[test]
fn the_largest_assignment_body_matches_a_measured_fullest_assignment() {
    let assignment = fixtures::fullest_assignment();
    // Fixed32 + workspace29 + save1 + live3 + charter20 + transcript20 + calls66
    // + messages53 + grants118 =342 (two full program names).
    assert_eq!(assignment.measure(), 342);
    assert_eq!(largest_body(&SIZES_ONE_SLOT, Kind::Assignment), Some(342));
}

#[test]
fn the_largest_grant_body_holds_a_four_kib_credential() {
    // The small profile's token bound.
    let sizes = Sizes { grant: 7934, ..SIZES_ONE_SLOT };
    // Task8 + attempt8 + number4 + generation8 + valid8 + arm1, and the
    // largest arm, credential: scheme1 + token7938 =7976.
    assert_eq!(largest_body(&sizes, Kind::Grant), Some(7976));
    let granted = fixtures::granted_key(&limits(&sizes).expect("the profile's bounds"), &[b'k'; 4096]);
    // Up to the arm 37 + scheme1 + key4100 =4138.
    assert_eq!(granted.measure(), 4138);
}

#[test]
fn sizes_past_a_ceiling_give_no_codec_limits() {
    for (field, sizes) in [
        ("program sources", Sizes { program_sources: 65_537, ..SIZES_ONE_SLOT }),
        ("charter", Sizes { charter: 67_108_865, ..SIZES_ONE_SLOT }),
        ("transcript", Sizes { transcript: 8_388_609, ..SIZES_ONE_SLOT }),
        ("piece", Sizes { piece: 4097, ..SIZES_ONE_SLOT }),
        ("turn", Sizes { turn: 8_388_609, ..SIZES_ONE_SLOT }),
        ("message", Sizes { message: 1_048_577, ..SIZES_ONE_SLOT }),
        ("sender", Sizes { sender: 1025, ..SIZES_ONE_SLOT }),
        ("call name", Sizes { call_name: 65, ..SIZES_ONE_SLOT }),
        ("tool name", Sizes { tool_name: 65, ..SIZES_ONE_SLOT }),
        ("call input", Sizes { call_input: 1_048_577, ..SIZES_ONE_SLOT }),
        ("answer", Sizes { answer: 1_048_577, ..SIZES_ONE_SLOT }),
        ("result", Sizes { result: 67_108_865, ..SIZES_ONE_SLOT }),
        ("detail", Sizes { detail: 4097, ..SIZES_ONE_SLOT }),
        ("grant", Sizes { grant: 8193, ..SIZES_ONE_SLOT }),
        ("name", Sizes { name: 257, ..SIZES_ONE_SLOT }),
        ("secret", Sizes { secret: 1025, ..SIZES_ONE_SLOT }),
        ("model name", Sizes { model_name: 129, ..SIZES_ONE_SLOT }),
        ("items", Sizes { items: 1_048_577, ..SIZES_ONE_SLOT }),
        ("left", Sizes { left: 65_537, ..SIZES_ONE_SLOT }),
        ("saved", Sizes { saved: 65_537, ..SIZES_ONE_SLOT }),
        ("evidence", Sizes { evidence: 1_048_577, ..SIZES_ONE_SLOT }),
        ("resource", Sizes { resource: 1025, ..SIZES_ONE_SLOT }),
        ("slots", Sizes { slots: 65_537, ..SIZES_ONE_SLOT }),
        ("inbox", Sizes { inbox: 1025, ..SIZES_ONE_SLOT }),
        ("answered", Sizes { answered: 1025, ..SIZES_ONE_SLOT }),
        ("models", Sizes { models: 65, ..SIZES_ONE_SLOT }),
        ("workstreams", Sizes { workstreams: 65_537, ..SIZES_ONE_SLOT }),
        ("accounts", Sizes { accounts: 64, ..SIZES_ONE_SLOT }),
        ("credentials", Sizes { credentials: 64, ..SIZES_ONE_SLOT }),
    ] {
        assert_eq!(limits(&sizes), None, "{field}");
    }
}

#[test]
fn a_refused_accounts_code_and_reason_take_the_details_bound_within_their_ceilings() {
    let small = limits(&SIZES_ONE_SLOT).expect("small size bounds");
    assert_eq!((small.account_refused_code, small.account_refused_reason), (16, 16));
    let large = limits(&Sizes { detail: 4096, ..SIZES_ONE_SLOT }).expect("the detail's ceiling");
    assert_eq!((large.account_refused_code, large.account_refused_reason), (128, 512));
}

#[test]
fn a_cap_past_a_u32_is_none() {
    assert!(limits(&SIZES_LARGE).is_some());
    assert_eq!(engine_cap(&SIZES_LARGE), None);
    assert_eq!(worker_cap(&SIZES_LARGE), None);
}

#[test]
fn a_grant_count_sum_that_overflows_has_no_codec_limits() {
    let sizes = Sizes { accounts: u32::MAX, credentials: 1, ..SIZES_ONE_SLOT };
    assert_eq!(limits(&sizes), None);
}

#[test]
fn the_facts_reserve_that_overflows_the_cap_is_refused() {
    let sizes = Sizes { facts: u32::MAX, ..SIZES_ONE_SLOT };
    assert_eq!(worker_cap(&sizes), None);
}
