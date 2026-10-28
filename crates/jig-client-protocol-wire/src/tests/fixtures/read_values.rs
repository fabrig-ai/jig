//! Every contract read variant with its existing document's smallest value.
use crate::*;
use skein_lib::{List, Reader};

macro_rules! value {
    ($rows:expr, $limits:expr, $variant:ident, $record:ident, $length:expr, $wrap:path) => {{
        let bytes = super::bytes($length);
        let record = $record::decode($limits, &mut Reader::new(&bytes)).expect("smallest existing document");
        $rows.push((stringify!($variant), ReadValue::$variant($wrap(record)))).expect("26 read variants");
    }};
    ($rows:expr, $limits:expr, $variant:ident, $record:ident, $length:expr) => {
        value!($rows, $limits, $variant, $record, $length, core::convert::identity);
    };
}

pub fn read_values(limits: &Limits) -> List<(&'static str, ReadValue)> {
    let mut values = List::with_capacity(26);
    value!(values, limits, OwnerProject, OwnerProject, super::OWNER_PROJECT_MIN_BYTES);
    value!(values, limits, Historical, OwnerHistorical, 289, Box::new);
    value!(values, limits, Me, Me, 79);
    value!(values, limits, Turn, ArchivedTurn, 42);
    value!(values, limits, Task, Task, 262);
    value!(values, limits, Proposal, Proposal, 149);
    value!(values, limits, OwnerProposal, OwnerProposal, 57);
    value!(values, limits, Escalation, Escalation, 216);
    value!(values, limits, OwnerEscalation, OwnerEscalation, 266);
    value!(values, limits, Question, Question, 31);
    value!(values, limits, PersonTask, PersonTask, 63);
    value!(values, limits, OwnerPersonTask, OwnerPersonTask, 71);
    value!(values, limits, Result, ResultDocument, 15);
    value!(values, limits, Ended, ObjectPage, 7);
    value!(values, limits, OwnerEnded, OwnerEndedPage, 7);
    value!(values, limits, History, HistoryPage, 7);
    value!(values, limits, Activity, ActivityPage, 7);
    value!(values, limits, Inbox, InboxPage, 7);
    value!(values, limits, OwnerInbox, OwnerInboxPage, 8);
    value!(values, limits, Spend, SpendPage, 7);
    value!(values, limits, OwnerSpend, OwnerSpendPage, 7);
    value!(values, limits, Project, Project, 159);
    value!(values, limits, Notes, NotePage, 7);
    value!(values, limits, Deployment, Deployment, 24);
    value!(values, limits, OwnerDeployment, OwnerDeployment, 148);
    values
}
