//! Bounds and unread evidence for parties' transcript reads.
use super::{
    Box, CHILD, Token, assert_empty, assert_too_linked, child_store, core, corrupt_disk, fixtures, one_row,
    party_store, party_submit, read_disk, read_result, reads, reply_range, turn_range, turn_reply, waiting_party,
};

#[test]
fn an_unread_turn_is_malformed() {
    let (mut store, mut disk) = party_store();
    corrupt_disk(&mut disk);
    let below = waiting_party(&mut store, turn_range());
    let reads::Loaded::TurnProblem { task, attempt, turn, owner, why } = read_disk(&mut store, below, &disk) else {
        panic!("malformed evidence")
    };
    assert_eq!((task, attempt, turn), (CHILD, 1, 2));
    assert_eq!(*owner, super::task_owner());
    assert_eq!(why, reads::TurnFailure::Malformed);
}

#[test]
fn a_turn_row_counts_its_joined_owner_against_the_byte_budget() {
    let (mut store, disk) = party_store();
    let mut range = turn_range();
    range.bytes = 1;
    let below = waiting_party(&mut store, range);
    let reads::Loaded::Busy = read_disk(&mut store, below, &disk) else { panic!("evidence must fit") };
}

#[test]
fn a_reply_turn_counts_against_the_join_budget() {
    // The reply's turn consumes the only join, before its owner.
    const TURN_JOINS: u32 = 1;
    let (mut store, disk) = party_store();
    store.limits.joins = TURN_JOINS;
    let below = waiting_party(&mut store, reply_range());
    let result = read_disk(&mut store, below, &disk);
    assert_too_linked(&result, TURN_JOINS);
}

#[test]
fn a_turns_point_cursor_excludes_the_named_turn() {
    let (mut store, _) = party_store();
    let mut range = turn_range();
    range.after =
        Some(reads::Position::Owner(core::Key::Core(core::CoreKey::Turn { task: CHILD, attempt: 1, turn: 2 })));
    let (events, below) = party_submit(&mut store, Token::new(7), range);
    assert_empty(&read_result(events));
    assert!(below.transcripts.is_empty());
}

#[test]
fn a_zero_row_party_read_does_not_start_io() {
    let (mut store, _) = party_store();
    for mut range in [turn_range(), reply_range()] {
        range.rows = 0;
        let (events, below) = party_submit(&mut store, Token::new(7), range);
        assert_empty(&read_result(events));
        assert!(below.transcripts.is_empty());
    }
}

#[test]
fn a_reply_with_no_transcript_place_returns_without_its_turn() {
    let mut store = child_store();
    fixtures::committed(&mut store, Box::new([fixtures::header(2), fixtures::save(turn_reply())]));
    let (events, below) = party_submit(&mut store, Token::new(7), reply_range());
    let result = read_result(events);
    assert_eq!(one_row(&result).related, None);
    assert!(below.transcripts.is_empty());
}

#[test]
fn an_oversized_turn_keeps_its_payload_size() {
    let (mut store, disk) = party_store();
    let below = waiting_party(&mut store, turn_range());
    let carried = read_disk(&mut store, below, &disk);
    // One byte below this exact complete row excludes the payload, while evidence fits.
    let row_bytes = one_row(&carried).owned_bytes().expect("row size");
    let mut range = turn_range();
    range.bytes = row_bytes.checked_sub(1).expect("complete row");
    let below = waiting_party(&mut store, range);
    let reads::Loaded::TurnProblem { task, attempt, turn, owner, why } = read_disk(&mut store, below, &disk) else {
        panic!("size evidence")
    };
    assert_eq!((task, attempt, turn), (CHILD, 1, 2));
    assert_eq!(*owner, super::task_owner());
    assert_eq!(why, reads::TurnFailure::TooLarge { bytes: 3 });
}

#[test]
fn a_turn_past_the_join_budget_names_its_bound() {
    // Even the owner's single join is unavailable.
    const NO_JOINS: u32 = 0;
    let (mut store, disk) = party_store();
    store.limits.joins = NO_JOINS;
    let below = waiting_party(&mut store, turn_range());
    let result = read_disk(&mut store, below, &disk);
    assert_too_linked(&result, NO_JOINS);
}

#[test]
fn an_absent_turn_is_an_empty_page() {
    for turn in [0, 3] {
        let (mut store, disk) = party_store();
        let mut range = turn_range();
        range.scope = reads::Scope::Turn { task: CHILD, attempt: 1, turn };
        let below = waiting_party(&mut store, range);
        assert_empty(&read_disk(&mut store, below, &disk));
    }
}
