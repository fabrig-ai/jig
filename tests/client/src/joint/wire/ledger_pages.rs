//! Whole historical delegate and period pages (client/domain/contract.md, 6;
//! pages.md, 5.2 and 6.5). Source selection belongs to the core; this boundary
//! retains row order, independent clocks and both original opaque edges.
use super::{chips, cursor, owner, pages::Invalid, results};
use jig_client::DomainLimits as Limits;
use jig_client_domain::contract as client;
use jig_client_scripted_engine as admission;
use jig_domain_core::reads;
use jig_domain_tasks as tasks;
use skein_lib::List;

fn edges(
    page: &reads::Page,
    scope: &reads::Scope,
    limits: &Limits,
) -> Result<(Option<client::Cursor>, Option<client::Cursor>), Invalid> {
    fn edge(
        value: Option<&reads::Cursor>,
        scope: &reads::Scope,
        limit: u32,
    ) -> Result<Option<client::Cursor>, Invalid> {
        value
            .map(|value| {
                if !cursor::belongs_to(value, scope) {
                    return Err(Invalid::Identity);
                }
                cursor::encode(value, limit).map_err(|_| Invalid::Cursor)
            })
            .transpose()
    }
    if page.rows.capacity() > limits.rows {
        return Err(Invalid::Owner(owner::Invalid::TooLarge));
    }
    Ok((edge(page.before.as_ref(), scope, limits.text)?, edge(page.next.as_ref(), scope, limits.text)?))
}
fn admitted(value: client::ReadValue, limits: &Limits) -> Result<client::ReadValue, Invalid> {
    admission::admit_read_value(
        &value,
        &admission::Bounds {
            text: limits.text,
            rows: limits.rows,
            authority: crate::test::AUTHORITY_ENTRIES,
            segments: limits.segments,
            document_bytes: limits.document_bytes,
            ..admission::Bounds::default()
        },
    )
    .map_err(|_| Invalid::Payload)?;
    Ok(value)
}

/// Copy whole already-selected direct delegate endings, independently of chip ending metadata.
/// # Errors
/// Refuses foreign edges/row kinds, reserved allocations or the whole receiving envelope.
pub fn ended(task: u64, page: &reads::Page, limits: &Limits) -> Result<client::ReadValue, Invalid> {
    let (before, after) = edges(page, &reads::Scope::EndedDelegates { task }, limits)?;
    let mut rows = List::with_capacity(page.rows.capacity());
    for row in &page.rows {
        let reads::PageRow::EndedDelegate(value) = row else {
            return Err(Invalid::Identity);
        };
        rows.push(client::OwnerEndedDelegate {
            chip: chips::chip(&value.chip, limits).map_err(Invalid::Owner)?,
            ended: results::ending(&value.ended, limits).map_err(Invalid::Owner)?,
            at: value.at,
        })
        .map_err(|_| Invalid::Owner(owner::Invalid::TooLarge))?;
    }
    admitted(
        client::ReadValue::OwnerEnded(Box::new(client::OwnerEndedPage { rows, before, after, cut: page.cut })),
        limits,
    )
}

/// Copy the original project's period ledger rows without converting them into live spend or pools.
/// # Errors
/// Refuses foreign/non-period sources, foreign edges, row kinds or any complete receiving envelope.
pub fn spend(project: u32, page: &reads::Page, limits: &Limits) -> Result<client::ReadValue, Invalid> {
    let (before, after) = edges(page, &reads::Scope::Spend { project }, limits)?;
    let mut rows = List::with_capacity(page.rows.capacity());
    for row in &page.rows {
        let reads::PageRow::Spend(value) = row else {
            return Err(Invalid::Identity);
        };
        if !matches!(value.funder, tasks::Funder::Period { project: owner, .. } if owner == project) {
            return Err(Invalid::Identity);
        }
        rows.push(client::OwnerFundingRecord {
            period: value.period.map(|value| client::OwnerPeriodTimes { began: value.began, next: value.next }),
            funder: owner::funder(value.funder),
            parent: value.parent.map(owner::funder),
            numbers: owner::numbers(value.numbers),
            made: value.made,
            closed: value.closed,
        })
        .map_err(|_| Invalid::Owner(owner::Invalid::TooLarge))?;
    }
    admitted(
        client::ReadValue::OwnerSpend(Box::new(client::OwnerSpendPage { rows, before, after, cut: page.cut })),
        limits,
    )
}

#[cfg(test)]
pub(crate) mod tests {
    const UTF8_TEXTS: [&[u8]; 2] = [b"~\0", b"\0~"];

    use super::*;
    use jig_domain_core::documents;
    use skein_lib::Wall;

    pub(crate) fn limits() -> Limits {
        Limits { rows: 8, text: 1024, turn_text: 1024, document_bytes: 100_000, ..crate::test::limits().client.domain }
    }
    fn page(scope: reads::Scope) -> reads::Page {
        let key = match scope {
            reads::Scope::EndedDelegates { .. } => tasks::Key::Ended(u64::MAX),
            reads::Scope::Spend { project } => tasks::Key::Ledger(tasks::Funder::Period { project, period: u64::MAX }),
            reads::Scope::Holds { .. }
            | reads::Scope::Task { .. }
            | reads::Scope::Proposal { .. }
            | reads::Scope::Escalation { .. }
            | reads::Scope::Question { .. }
            | reads::Scope::PersonTask { .. }
            | reads::Scope::Result { .. }
            | reads::Scope::Reply { .. }
            | reads::Scope::Turn { .. }
            | reads::Scope::History { .. }
            | reads::Scope::Activity { .. }
            | reads::Scope::Inbox { .. }
            | reads::Scope::Tasks { .. }
            | reads::Scope::Notes { .. }
            | reads::Scope::SettingsHistory { .. }
            | reads::Scope::ScopedNotes { .. } => panic!("one of the two historical scopes"),
        };
        reads::Page {
            rows: List::with_capacity(8),
            before: Some(reads::Cursor::Store { scope: scope.clone(), after: None }),
            next: Some(reads::Cursor::Store {
                scope,
                after: Some(reads::Position::Owner(jig_domain_core::Key::Tasks(key))),
            }),
            cut: true,
            inbox_selection: None,
        }
    }
    pub(crate) fn ended_source() -> reads::Page {
        let mut page = page(reads::Scope::EndedDelegates { task: u64::MAX });
        for (index, ended) in [
            documents::Ending::Done(documents::ResultValue::Report(documents::Text {
                bytes: Box::from(UTF8_TEXTS[0]),
                cut: true,
            })),
            documents::Ending::Failed(documents::Text { bytes: Box::new([]), cut: false }),
            documents::Ending::Cancelled {
                reason: documents::Text { bytes: Box::from(UTF8_TEXTS[1]), cut: true },
                result: Some(documents::ResultValue::Approved { state: Box::from([255, 0]), words: None }),
                by: tasks::Party::Deployment { project: u32::MAX },
                at: Wall::EPOCH,
            },
            documents::Ending::Cancelled {
                reason: documents::Text { bytes: Box::new([]), cut: false },
                result: None,
                by: tasks::Party::Task(0),
                at: Wall::from_nanos(u64::MAX),
            },
        ]
        .into_iter()
        .enumerate()
        {
            let mut chip = super::super::chips::tests::source_chip();
            chip.number = u64::try_from(index).expect("four supplied rows");
            page.rows
                .push(reads::PageRow::EndedDelegate(documents::EndedDelegate {
                    chip,
                    ended,
                    at: if index == 1 { Some(Wall::EPOCH) } else { None },
                }))
                .expect("whole selected delegate");
        }
        page
    }
    pub(crate) fn spend_source() -> reads::Page {
        let mut page = page(reads::Scope::Spend { project: u32::MAX });
        for (index, parent) in [
            None,
            Some(tasks::Funder::Task(u64::MAX)),
            Some(tasks::Funder::Pool { project: 0, person: u64::MAX, period: 0 }),
            Some(tasks::Funder::Recurring { project: u32::MAX, task: 0, period: u64::MAX }),
            Some(tasks::Funder::Period { project: 0, period: u64::MAX }),
        ]
        .into_iter()
        .enumerate()
        {
            page.rows
                .push(reads::PageRow::Spend(tasks::FundingRecord {
                    period: if index == 0 {
                        None
                    } else {
                        Some(tasks::PeriodTimes {
                            began: Wall::EPOCH,
                            next: if index == 1 { None } else { Some(Wall::from_nanos(u64::MAX)) },
                        })
                    },
                    funder: tasks::Funder::Period {
                        project: u32::MAX,
                        period: u64::MAX - u64::try_from(index).expect("five supplied rows"),
                    },
                    parent,
                    numbers: tasks::Numbers { budget: 0, spent: u64::MAX, spent_below: 17, reserved: 29 },
                    made: u32::MAX,
                    closed: index != 0,
                }))
                .expect("whole selected period");
        }
        page
    }
    #[test]
    fn ended_pages_keep_whole_chips_all_endings_independent_clocks_order_and_both_original_edges() {
        let source = ended_source();
        let original = source.clone();
        let l = limits();
        let client::ReadValue::OwnerEnded(value) = ended(u64::MAX, &source, &l).expect("whole historical delegates")
        else {
            panic!("native delegate page")
        };
        assert_eq!((value.rows.len(), value.rows.capacity()), (4, 8));
        assert!(value.cut);
        assert_eq!(cursor::decode(value.before.as_ref().expect("supplied before"), l.text), source.before);
        assert_eq!(cursor::decode(value.after.as_ref().expect("supplied after"), l.text), source.next);
        for (source, actual) in source.rows.iter().zip(value.rows.iter()) {
            let reads::PageRow::EndedDelegate(source) = source else { panic!("supplied delegate") };
            assert_eq!(actual.chip, chips::chip(&source.chip, &l).expect("whole metadata"));
            assert_eq!(actual.ended, results::ending(&source.ended, &l).expect("actual ending"));
            assert_eq!(actual.at, source.at);
            assert!(actual.chip.title.cut);
            let client::ChipPhase::Owner { details, .. } = actual.chip.details.as_ref() else {
                panic!("whole metadata")
            };
            assert!(details.ending.is_none());
            assert_eq!(details.ended_at, Some(Wall::EPOCH));
        }
        assert!(matches!(
            value.rows.get(2).expect("cancelled result").ended,
            client::OwnerChipEnding::Cancelled {
                result: Some(_),
                by: client::OwnerRequester::Deployment { project: u32::MAX },
                ..
            }
        ));
        assert!(matches!(
            value.rows.get(3).expect("absent prior result").ended,
            client::OwnerChipEnding::Cancelled { result: None, .. }
        ));
        assert_eq!(ended(0, &source, &l), Err(Invalid::Identity));
        assert_eq!(source, original);
    }
    #[test]
    fn spend_pages_keep_exact_period_selection_optional_boundaries_all_parent_kinds_and_four_counters() {
        let source = spend_source();
        let original = source.clone();
        let l = limits();
        let client::ReadValue::OwnerSpend(value) = spend(u32::MAX, &source, &l).expect("whole selected spend") else {
            panic!("native funding page")
        };
        assert_eq!((value.rows.len(), value.rows.capacity()), (5, 8));
        assert!(value.cut);
        assert_eq!(cursor::decode(value.before.as_ref().expect("supplied before"), l.text), source.before);
        assert_eq!(cursor::decode(value.after.as_ref().expect("supplied after"), l.text), source.next);
        for (source, actual) in source.rows.iter().zip(value.rows.iter()) {
            let reads::PageRow::Spend(source) = source else { panic!("source ledger") };
            assert_eq!(actual.funder, owner::funder(source.funder));
            assert_eq!(actual.parent, source.parent.map(owner::funder));
            assert_eq!(
                actual.numbers,
                client::OwnerNumbers { budget: 0, spent: u64::MAX, spent_below: 17, reserved: 29 }
            );
            assert_eq!(actual.made, u32::MAX);
            assert_eq!(actual.closed, source.closed);
            assert_eq!(actual.period, source.period.map(|p| client::OwnerPeriodTimes { began: p.began, next: p.next }));
        }
        for funder in [
            tasks::Funder::Task(0),
            tasks::Funder::Pool { project: u32::MAX, person: 0, period: 0 },
            tasks::Funder::Recurring { project: u32::MAX, task: 0, period: 0 },
            tasks::Funder::Period { project: 0, period: 0 },
        ] {
            let mut wrong = source.clone();
            let reads::PageRow::Spend(row) = wrong.rows.get_mut(4).expect("last selected row") else {
                panic!("ledger")
            };
            row.funder = funder;
            assert_eq!(spend(u32::MAX, &wrong, &l), Err(Invalid::Identity));
        }
        assert_eq!(spend(0, &source, &l), Err(Invalid::Identity));
        assert_eq!(source, original);
    }
    fn whole_bounds(source: &reads::Page, translate: fn(&reads::Page, &Limits) -> Result<client::ReadValue, Invalid>) {
        let original = source.clone();
        let l = limits();
        assert_eq!(
            translate(source, &Limits { rows: 7, turn_text: l.turn_text, ..l }),
            Err(Invalid::Owner(owner::Invalid::TooLarge))
        );
        assert_eq!(translate(source, &Limits { text: 1, turn_text: 1, ..l }), Err(Invalid::Cursor));
        let mut wrong = source.clone();
        wrong.before = Some(reads::Cursor::Store { scope: reads::Scope::Activity { task: 0 }, after: None });
        assert_eq!(translate(&wrong, &l), Err(Invalid::Identity));
        wrong = source.clone();
        wrong.next = Some(reads::Cursor::Store { scope: reads::Scope::Activity { task: 0 }, after: None });
        assert_eq!(translate(&wrong, &l), Err(Invalid::Identity));
        wrong = source.clone();
        *wrong.rows.get_mut(0).expect("first selected row") = reads::PageRow::Spend(tasks::FundingRecord {
            period: None,
            funder: tasks::Funder::Task(0),
            parent: None,
            numbers: tasks::Numbers { budget: 0, spent: 0, spent_below: 0, reserved: 0 },
            made: 0,
            closed: false,
        });
        assert_eq!(translate(&wrong, &l), Err(Invalid::Identity));
        let (mut refused, mut accepted) = (0, l.document_bytes);
        assert_eq!(
            translate(source, &Limits { document_bytes: 0, turn_text: l.turn_text, ..l }),
            Err(Invalid::Payload)
        );
        while accepted - refused > 1 {
            let bytes = refused + (accepted - refused) / 2;
            if translate(source, &Limits { document_bytes: bytes, turn_text: l.turn_text, ..l }).is_ok() {
                accepted = bytes;
            } else {
                refused = bytes;
            }
        }
        assert!(translate(source, &Limits { document_bytes: accepted, turn_text: l.turn_text, ..l }).is_ok());
        assert_eq!(
            translate(source, &Limits { document_bytes: refused, turn_text: l.turn_text, ..l }),
            Err(Invalid::Payload)
        );
        let mut empty = source.clone();
        empty.rows = List::with_capacity(8);
        assert_eq!(
            translate(&empty, &Limits { rows: 7, turn_text: l.turn_text, ..l }),
            Err(Invalid::Owner(owner::Invalid::TooLarge))
        );
        assert!(translate(&empty, &l).is_ok());
        assert_eq!(source, &original);
    }
    #[test]
    fn ended_page_admission_prices_whole_reserved_rows_nested_metadata_and_both_edges_without_clipping() {
        whole_bounds(&ended_source(), |s, l| ended(u64::MAX, s, l));
        let mut source = ended_source();
        source.before = None;
        source.next = None;
        assert_eq!(
            ended(u64::MAX, &source, &Limits { text: 1, turn_text: 1, ..limits() }),
            Err(Invalid::Owner(owner::Invalid::TooLarge))
        );
        let reads::PageRow::EndedDelegate(last) = source.rows.get_mut(3).expect("last whole delegate") else {
            panic!("delegate")
        };
        last.chip.details.resources = List::with_capacity(9);
        let unchanged = source.clone();
        assert_eq!(ended(u64::MAX, &source, &limits()), Err(Invalid::Owner(owner::Invalid::TooLarge)));
        assert_eq!(source, unchanged);
    }
    #[test]
    fn spend_page_admission_prices_whole_reserved_rows_and_both_edges_without_clipping() {
        whole_bounds(&spend_source(), |s, l| spend(u32::MAX, s, l));
    }
}
