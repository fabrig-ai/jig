//! Settings values cost every field and owned list capacity (contract.md, 5–6).
use super::Payload;
use crate::Bounds;
use jig_client_domain::contract::{
    ChangeSettings, Charter, HeldRevision, RevisionAnswer, RevisionDecider, RevisionDocument, RevisionEdit,
    RevisionHead, RevisionItem, RevisionItemEdit, RevisionItemHead, RevisionParty, RevisionRow, RevisionsPage, Rules,
    Running, Setting, SettingDocument, SettingEdit, SettingFollows, SettingHosts, SettingItem, SettingLine,
    SettingModel, SettingNumber, SettingPolicy, SettingSet, SettingValue, SettingsDocument, SettingsProposal,
};
plain!(SettingNumber, Setting, SettingHosts, Running, SettingFollows, RevisionParty, RevisionDecider, RevisionItemEdit);
fields!(SettingModel {
    dialect,
    endpoint,
    program,
    name,
    window,
    output,
    input_price,
    cached_price,
    cache_write_price,
    output_price,
    price_unit,
    efforts,
    effort
});
fields!(Charter {
    tools,
    number,
    instructions,
    waiting,
    resume,
    turns,
    time,
    reserve_turns,
    reserve_time,
    model,
    alternatives,
    inspect,
    modify,
    shell,
    agents,
    call_timeout,
    hosts,
    spend
});
fields!(SettingPolicy {
    escalation_role,
    ceiling,
    projections,
    period_spend,
    roles,
    requirements,
    permissions,
    templates
});
fields!(Rules { ceiling, period_spend, minimum_run_spend, maximum_run_spend, implies, requirements });
fields!(SettingSet { value });
fields!(SettingItem { setting, edit });
fields!(ChangeSettings { against, items, running });
fields!(SettingLine { setting, name, follows, last });
fields!(RevisionHead { number, against, at, by, proposal, moved });
fields!(HeldRevision { head, holders });
fields!(SettingsProposal { proposer, number });
fields!(SettingsDocument { newest, settings, held, pending });
fields!(SettingDocument { setting, revision, value });
fields!(RevisionItem { setting, edit });
fields!(RevisionDocument { head, items });
fields!(RevisionItemHead { setting, edit });
fields!(RevisionRow { head, items, more });
fields!(RevisionsPage { rows, next, forgotten });
impl Payload for SettingValue {
    fn heap(&self, bounds: &Bounds) -> Option<u64> {
        match self {
            Self::Charter(v) => v.heap(bounds),
            Self::Template(v) => v.heap(bounds),
            Self::Policy(v) => v.heap(bounds),
            Self::Rules(v) => v.heap(bounds),
        }
    }
}
impl Payload for SettingEdit {
    fn heap(&self, bounds: &Bounds) -> Option<u64> {
        match self {
            Self::Set(v) => v.heap(bounds),
            Self::Removed | Self::Reset => Some(0),
        }
    }
}
impl Payload for RevisionEdit {
    fn heap(&self, bounds: &Bounds) -> Option<u64> {
        match self {
            Self::Set(v) => v.heap(bounds),
            Self::TooLarge(_) | Self::Removed | Self::Reset => Some(0),
        }
    }
}
impl Payload for RevisionAnswer {
    fn heap(&self, bounds: &Bounds) -> Option<u64> {
        match self {
            Self::Revision(v) => v.heap(bounds),
            Self::Forgotten(_) | Self::Unknown => Some(0),
        }
    }
}

use jig_client_domain::contract::{
    OriginRevision, OwnerProposalActionSettings, OwnerRetainedActionSettings, ProposeSettings, ReviewItem, ReviewValue,
    SettingOrigin, SettingsReview,
};
plain!(OriginRevision, SettingOrigin);
fields!(ReviewItem { setting, edit, now, origin, would_be });
impl Payload for ReviewValue {
    fn heap(&self, bounds: &Bounds) -> Option<u64> {
        match self {
            Self::Value(value) => value.heap(bounds),
            Self::TooLarge(_) | Self::None => Some(0),
        }
    }
}
impl Payload for ProposeSettings {
    fn heap(&self, bounds: &Bounds) -> Option<u64> {
        self.items.heap(&Bounds { rows: 128, ..*bounds })
    }
}
impl Payload for SettingsReview {
    fn heap(&self, bounds: &Bounds) -> Option<u64> {
        self.items.heap(&Bounds { rows: 128, ..*bounds })
    }
}
impl Payload for OwnerProposalActionSettings {
    fn heap(&self, bounds: &Bounds) -> Option<u64> {
        self.items.heap(&Bounds { rows: 128, ..*bounds })?.checked_add(self.review.heap(bounds)?)
    }
}
impl Payload for OwnerRetainedActionSettings {
    fn heap(&self, bounds: &Bounds) -> Option<u64> {
        self.items.heap(&Bounds { rows: 128, ..*bounds })
    }
}
