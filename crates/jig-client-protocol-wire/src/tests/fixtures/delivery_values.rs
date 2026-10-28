//! Every declared delivery variant with its document's smallest value.
use crate::*;
use skein_lib::{List, Reader};

macro_rules! value {
    ($rows:expr, $limits:expr, $variant:ident, $record:ident, $length:expr, $wrap:path) => {{
        let bytes = super::bytes($length);
        let record = $record::decode($limits, &mut Reader::new(&bytes)).expect("smallest existing document");
        $rows.push((stringify!($variant), DeliveryValue::$variant($wrap(record)))).expect("reserved delivery variants");
    }};
    ($rows:expr, $limits:expr, $variant:ident, $record:ident, $length:expr) => {
        value!($rows, $limits, $variant, $record, $length, core::convert::identity);
    };
}

pub fn delivery_values(limits: &Limits) -> List<(&'static str, DeliveryValue)> {
    let mut values = List::with_capacity(16);
    value!(values, limits, OwnerProject, OwnerProject, super::OWNER_PROJECT_MIN_BYTES);
    value!(values, limits, Me, Me, 79);
    value!(values, limits, Task, Task, 262, Box::new);
    value!(values, limits, OwnerProposal, OwnerProposal, 57);
    value!(values, limits, Tree, Tree, 48);
    value!(values, limits, OwnerTree, OwnerTree, 17);
    value!(values, limits, Chips, Chips, 4);
    value!(values, limits, Inbox, Inbox, 38);
    value!(values, limits, OwnerInbox, OwnerInbox, 36);
    value!(values, limits, Tasks, Tasks, 5);
    value!(values, limits, Run, RunText, 40);
    value!(values, limits, RunState, RunState, 15);
    value!(values, limits, Deployment, Deployment, 24);
    value!(values, limits, OwnerDeployment, OwnerDeployment, 148);
    values
}
