use super::fixtures;
use crate::translate::pages;
use jig_client_protocol_wire::v19 as w;
use jig_domain_core::reads;

fn activity(read: &reads::Page, limits: &w::Limits) -> Result<w::ReadValue, w::Problem> {
    pages::page(&fixtures::models(), &reads::Read::Activity { task: 7, before: None }, read, limits)
}
#[test]
fn an_activity_page_carries_each_turn_s_bytes() {
    let limits = fixtures::wire_limits();
    let source = fixtures::activity_page();
    assert_eq!(
        activity(&source, &limits),
        Ok(w::ReadValue::Activity(jig_client_test_records::carried_activity_page()))
    );
    for (label, limits, path) in [
        ("turn bytes", w::Limits { turn_carried_bytes: 2, ..limits }, w::Path::TurnCarriedBytes),
        ("page pieces", w::Limits { activity_page_rows: 0, ..limits }, w::Path::ActivityPageRows),
        ("opaque cursor", w::Limits { cursor_value: 1, ..limits }, w::Path::CursorValue),
    ] {
        assert_eq!(activity(&source, &limits), Err(w::Problem { path, reason: skein_codec::Reason::Bound }), "{label}");
    }
}
#[test]
fn a_turn_past_the_page_crosses_as_its_size() {
    let limits = fixtures::wire_limits();
    let source = fixtures::oversized_turn_page();
    assert_eq!(
        activity(&source, &limits),
        Ok(w::ReadValue::Activity(jig_client_test_records::oversized_activity_page()))
    );
}
#[test]
fn a_history_page_keeps_its_cut_flag() {
    let limits = fixtures::wire_limits();
    let mut source = fixtures::history_page();
    let literal = jig_client_test_records::mixed_history_page();
    for (label, cut) in [("cut", true), ("whole", false)] {
        source.cut = cut;
        let mut expected = literal.clone().into_parts();
        expected.cut = cut;
        assert_eq!(
            pages::page(
                &fixtures::models(),
                &reads::Read::History { task: 7, subtree: false, from: None },
                &source,
                &limits
            ),
            Ok(w::ReadValue::History(w::HistoryPage::new(&limits, expected).expect(label))),
            "{label}"
        );
    }
}
#[test]
fn an_inbox_page_keeps_its_two_groups_apart() {
    let limits = fixtures::wire_limits();
    let needs = fixtures::inbox_page(false);
    let updates = fixtures::inbox_page(true);
    let records = jig_client_test_records::selected_inbox_pages();
    for (label, group, source, record) in [
        ("needs", reads::Group::Needs, &needs, records[0].clone()),
        ("updates", reads::Group::Updates, &updates, records[1].clone()),
    ] {
        let read = reads::Read::Inbox { group, filter: reads::Filter { project: None, goal: None }, from: None };
        assert_eq!(
            pages::page(&fixtures::models(), &read, source, &limits),
            Ok(w::ReadValue::OwnerInbox(record)),
            "{label}"
        );
    }
}
#[test]
fn a_notes_page_becomes_its_record() {
    let limits = fixtures::wire_limits();
    let source = fixtures::notes_page();
    let read = reads::Read::Notes { project: 13, from: None };
    assert_eq!(
        pages::page(&fixtures::models(), &read, &source, &limits),
        Ok(w::ReadValue::Notes(jig_client_test_records::scoped_notes_page()))
    );
}

#[test]
fn kept_lines_and_met_notices_cross_both_client_translations() {
    use jig_client_domain::contract as c;
    use jig_domain_tasks as t;
    use skein_lib::{List, Reader, Wall, Writer};
    let limits = fixtures::wire_limits();
    for removed in [false, true] {
        let mut rows = List::with_capacity(2);
        for (position, line) in [
            (13, t::Line::Kept { attempt: 3, pending: Box::new([19, 23]), question: None }),
            (
                11,
                t::Line::Heard(t::Word {
                    number: 17,
                    from: t::Party::Task(29),
                    kind: t::MessageKind::Met { dependency: 19, removed },
                    words: Box::new([]),
                    at: Wall::from_nanos(31),
                    hits: 1,
                    eligible: true,
                }),
            ),
        ] {
            rows.push(reads::PageRow::Activity {
                line: t::ActivityLine { task: 7, position, at: Wall::from_nanos(37), line },
                turn: None,
            })
            .unwrap();
        }
        let w::ReadValue::Activity(record) = activity(&fixtures::page(rows), &limits).unwrap() else {
            panic!("activity page");
        };
        let mut out = Writer::new(usize::try_from(record.measure()).unwrap());
        record.encode(&mut out).unwrap();
        let bytes = out.finish();
        let record = w::ActivityPage::decode(&limits, &mut Reader::new(&bytes)).unwrap();
        let actual =
            jig_client_protocol_link::translate::pages::activity_page(record, &fixtures::client_limits()).unwrap();
        let c::Activity::Owner { source, decoded } = &actual.rows.get(0).unwrap().event else { panic!("owner") };
        assert!(decoded.is_none());
        let c::OwnerActivity::Kept { attempt, pending, question: None } = &source.line else { panic!("kept") };
        assert_eq!((*attempt, pending.as_slice()), (3, [19, 23].as_slice()));
        let c::Activity::Owner { source, .. } = &actual.rows.get(1).unwrap().event else { panic!("owner") };
        let c::OwnerActivity::Heard(word) = &source.line else { panic!("heard") };
        assert_eq!(word.kind, c::OwnerMessageKind::Met { dependency: 19, removed });
        assert_eq!(word.from, c::OwnerRequester::Task(29));
    }
}
