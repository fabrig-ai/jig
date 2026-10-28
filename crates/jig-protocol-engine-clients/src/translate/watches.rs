#![expect(clippy::manual_map, reason = "step code translates optional watch facts without closures")]
//! Watches retain their whole subject and stream record (protocol/client.md, 7).
use super::{chips, cursor, deployment, inbox, live, objects, owner, task, w};
use alloc::boxed::Box;
use jig_domain_core::{self as core, documents as d};
use jig_domain_views as views;
use skein_lib::{List, Reader, Token, Writer};

pub(crate) fn watch(watcher: Token, sign_in: u64, value: w::Subject, rows: u32) -> Option<core::Event> {
    if let w::Subject::Run(value) = value {
        let value = value.into_parts();
        let turn = match value.turn {
            Some(value) => {
                Some(views::TurnName { attempt: value.attempt(), turn: value.number(), retry: value.retry() })
            }
            None => None,
        };
        return Some(core::Event::WatchRun { watcher, sign_in, task: value.task, turn, offset: value.offset });
    }
    let (subject, rows) = match value {
        w::Subject::Me => (d::Watched::Frame, rows),
        w::Subject::Inbox(value) => (d::Watched::Inbox, value.window()),
        w::Subject::Task(value) => (d::Watched::Task { task: value.task() }, rows),
        w::Subject::Tree(value) => (d::Watched::Tree { task: value.task() }, value.window()),
        w::Subject::Chips(value) => (d::Watched::Chips { tasks: value.into_parts().tasks }, rows),
        w::Subject::Tasks(value) => {
            let value = value.into_parts();
            (d::Watched::Query { query: super::queries::query(value.query)? }, value.window)
        }
        w::Subject::Accounts => (d::Watched::Accounts, rows),
        w::Subject::Deployment => (d::Watched::Deployment, rows),
        w::Subject::Settings => (d::Watched::Settings, rows),
        w::Subject::Run(_) => return None,
    };
    Some(core::Event::Watch { watcher, sign_in, subject, window: d::Window { rows, after: None } })
}
pub(crate) fn decode(bytes: &[u8], limits: &w::Limits) -> Option<w::Subject> {
    let mut reader = Reader::new(bytes);
    let value = w::Subject::decode(limits, &mut reader).ok()?;
    if !reader.is_empty() {
        return None;
    }
    Some(value)
}
pub(crate) fn ended(why: views::End) -> w::WatchEnd {
    match why {
        views::End::Unwatched => w::WatchEnd::Closed,
        views::End::Gone => w::WatchEnd::SubjectEnded,
        views::End::TooLinked { joins } => w::WatchEnd::TooLinked(
            w::WatchTooLinked::new(&w::CEILINGS, w::WatchTooLinkedParts { joins }).expect("no bounded field"),
        ),
        views::End::NotAllowed => w::WatchEnd::NotAllowed,
        views::End::SignedOut => w::WatchEnd::SignedOut,
        views::End::Stopping => w::WatchEnd::Stopping,
    }
}
pub(crate) fn refused(why: core::WatchRefusal) -> w::WatchRefusal {
    match why {
        core::WatchRefusal::Unknown => w::WatchRefusal::Unknown,
        core::WatchRefusal::NotAllowed => w::WatchRefusal::NotAllowed,
        core::WatchRefusal::Busy => w::WatchRefusal::Busy,
        core::WatchRefusal::SignedOut => w::WatchRefusal::SignedOut,
    }
}
pub(crate) fn hello(
    name: [u8; 16],
    settings: &crate::Settings,
    limits: &w::Limits,
) -> Result<w::StreamEvent, w::Problem> {
    let mut served = List::with_capacity(settings.served.len());
    for range in &settings.served {
        let owner = match range.family {
            None => w::Owner::Jig,
            Some(number) => w::Owner::Family(w::Family::new(limits, w::FamilyParts { number })?),
        };
        served
            .push(w::Served::new(limits, w::ServedParts { owner, first: range.first, last: range.last })?)
            .expect("served ranges fit");
    }
    Ok(w::StreamEvent::Hello(w::Hello::new(
        limits,
        w::HelloParts { stream: name, served, heartbeat: settings.heartbeat },
    )?))
}
pub(crate) fn delivery(
    value: &d::Document,
    _project: u32,
    rows: u32,
    models: &[crate::Model],
    limits: &w::Limits,
) -> Result<w::DeliveryValue, w::Problem> {
    Ok(match value {
        d::Document::Frame(value) => w::DeliveryValue::Me(owner::frame(value, models, limits)?),
        d::Document::Task(value) => w::DeliveryValue::Task(Box::new(task::document(value, models, limits)?)),
        d::Document::Tree(value) => {
            let next = match &value.next {
                Some(next) => Some(w::Cursor::new(
                    limits,
                    w::CursorParts {
                        value: cursor::encode_tree(next, value.root.number, rows, limits.cursor_value)
                            .ok_or(super::problem(skein_codec::Reason::Bound))?,
                    },
                )?),
                None => None,
            };
            w::DeliveryValue::OwnerTree(live::tree(value, next, limits)?)
        }
        d::Document::Chips(value) => {
            let mut rows = List::with_capacity(value.len());
            for chip in value {
                rows.push(chips::chip(chip, limits)?).expect("supplied chips fit");
            }
            let mut pieces = List::with_capacity(rows.len());
            for row in rows.into_boxed() {
                let mut bytes = Writer::new(usize::try_from(row.measure()).expect("chip measure fits"));
                row.encode(&mut bytes).expect("measured chip");
                pieces.push(bytes.finish()).expect("chip pieces fit");
            }
            let rows = super::pieces(&pieces, limits.chips_rows, w::Path::ChipsRows)?;
            w::DeliveryValue::Chips(w::Chips::new(limits, w::ChipsParts { rows })?)
        }
        d::Document::Inbox(value) => w::DeliveryValue::OwnerInbox(inbox::document(value, limits)?),
        d::Document::Lines(value) => w::DeliveryValue::Tasks(super::queries::lines(value, limits)?),
        d::Document::Run(value) => w::DeliveryValue::RunState(live::run(value, limits)?),
        d::Document::Accounts(value) => w::DeliveryValue::OwnerAccounts(super::accounts::document(value, limits)?),
        d::Document::Settings(value) => w::DeliveryValue::Settings(super::settings::document(value, limits)?),
        d::Document::Deployment(value) => w::DeliveryValue::OwnerDeployment(deployment::document(value, limits)?),
        d::Document::Proposal(value) => w::DeliveryValue::OwnerProposal(objects::proposal(models, value, limits)?),
        d::Document::Escalation(_)
        | d::Document::Question(_)
        | d::Document::PersonTask(_)
        | d::Document::Result(_)
        | d::Document::Project(_)
        | d::Document::Note(_) => return Err(super::problem(skein_codec::Reason::Tag)),
    })
}
pub(crate) fn owned(value: w::DeliveryValue, version: u32, limits: &w::Limits) -> Result<w::Owned, w::Problem> {
    let mut record = Writer::new(usize::try_from(value.measure()).expect("wire measure fits"));
    value.encode(&mut record).expect("measured delivery");
    w::Owned::new(limits, w::OwnedParts { owner: w::Owner::Jig, version, record: record.finish() })
}
pub(crate) fn packet(value: &w::StreamEvent) -> Box<[u8]> {
    let mut record = Writer::new(usize::try_from(value.measure()).expect("wire measure fits"));
    value.encode(&mut record).expect("measured stream record");
    record.finish()
}

pub(crate) fn document_packet(
    document: &d::Document,
    project: u32,
    rows: u32,
    watch: u64,
    version: u32,
    models: &[crate::Model],
    limits: &w::Limits,
) -> Result<w::StreamEvent, w::Problem> {
    let value = delivery(document, project, rows, models, limits)?;
    let document = owned(value, version, limits)?;
    Ok(w::StreamEvent::Delivery(w::Delivery::new(limits, w::DeliveryParts { watch, document })?))
}
pub(crate) fn application_packet(
    family: u16,
    version: u32,
    record: Box<[u8]>,
    watch: u64,
    limits: &w::Limits,
) -> Result<w::StreamEvent, w::Problem> {
    let family = w::Family::new(limits, w::FamilyParts { number: family })?;
    let document = w::Owned::new(limits, w::OwnedParts { owner: w::Owner::Family(family), version, record })?;
    Ok(w::StreamEvent::Delivery(w::Delivery::new(limits, w::DeliveryParts { watch, document })?))
}
pub(crate) fn text_packet(
    writing: &views::Writing,
    watch: u64,
    limits: &w::Limits,
) -> Result<w::StreamEvent, w::Problem> {
    let value = live::writing(writing, limits)?;
    Ok(w::StreamEvent::Text(w::StreamText::new(limits, w::StreamTextParts { watch, value })?))
}
pub(crate) fn end_packet(why: views::End, watch: u64, limits: &w::Limits) -> Result<w::StreamEvent, w::Problem> {
    Ok(w::StreamEvent::Ended(w::Ended::new(limits, w::EndedParts { watch, why: ended(why) })?))
}
