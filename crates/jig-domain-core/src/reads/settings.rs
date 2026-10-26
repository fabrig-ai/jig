//! Dense revision reads (engine.md, 17.7; client/domain/contract.md, section 6).
use super::{
    Answer, Range, RevisionAnswer, RevisionEdit, RevisionItemHead, RevisionRow, RevisionsPage, Row, Scope,
    SettingDocument, Why, terminal,
};
use crate::{Core, CoreRecord, Limits, Record, Requests, settings};
use alloc::boxed::Box;
use jig_domain_people as people;
use skein_lib::{Env, List, Map, ReplyTo};

#[must_use]
pub fn oldest(core: &Core, limits: &Limits) -> u32 {
    core.current.oldest().max(core.current.newest().saturating_sub(limits.settings.history)).max(1)
}
/// Whole charter values are available while their revision is held.
#[must_use]
pub fn may_read(core: &Core, revision: u32) -> bool {
    core.held(revision)
}

/// Run reads use the same documents and load slots as party reads (engine.md, 7.3).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum SettingsRead {
    Inventory,
    Setting { setting: settings::Setting, revision: u32 },
    Revision { number: u32 },
    Revisions { before: Option<u32> },
}
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum SettingsAnswer {
    Inventory(crate::documents::Settings),
    Setting(SettingDocument),
    Revision(RevisionAnswer),
    Revisions(RevisionsPage),
    Unknown,
}

struct Recipient {
    destination: super::Destination,
    sign_in: u64,
    party: u64,
}

pub(crate) fn run(
    core: &mut Core,
    env: &Env<Limits>,
    to: ReplyTo,
    key: crate::CallKey,
    read: SettingsRead,
) -> Requests {
    if let Some(part) = core.authorize_tool(key, crate::ToolKind::Settings) {
        return refused(to, key, part);
    }
    let recipient = Recipient { destination: super::Destination::Settings { to, key }, sign_in: 0, party: 0 };
    match read {
        SettingsRead::Inventory => answer(
            recipient.destination,
            Answer::Document(Box::new(crate::documents::Document::Settings(Box::new(
                crate::documents::settings_inventory(core),
            )))),
        ),
        SettingsRead::Setting { setting, revision } => {
            answer(recipient.destination, setting_value(core, setting, revision))
        }
        SettingsRead::Revision { number } => revision_read(core, env, recipient, number),
        SettingsRead::Revisions { before } => revisions_read(core, env, recipient, before),
    }
}

fn refused(to: ReplyTo, key: crate::CallKey, part: crate::CallPart) -> Requests {
    let mut out = skein_lib::Queue::with_capacity(2);
    out.push(crate::Request::Now(Box::new(crate::Now::EffectAnswer { to, key, part })));
    out.push(crate::Request::Decided);
    Requests::Out(out)
}

pub(super) fn answer(destination: super::Destination, value: Answer) -> Requests {
    let super::Destination::Settings { to, key } = destination else {
        let super::Destination::Read(reply) = destination else { unreachable!("settings read destination") };
        return terminal(reply, value);
    };
    let value = match value {
        Answer::Document(document) => {
            let crate::documents::Document::Settings(value) = *document else { unreachable!("settings inventory") };
            SettingsAnswer::Inventory(*value)
        }
        Answer::Setting(value) => SettingsAnswer::Setting(*value),
        Answer::Revision(value) => SettingsAnswer::Revision(*value),
        Answer::Revisions(value) => SettingsAnswer::Revisions(*value),
        Answer::None(Why::Busy) => return refused(to, key, crate::CallPart::Unavailable),
        Answer::None(_) => SettingsAnswer::Unknown,
        Answer::Tasks(_) | Answer::Historical(_) | Answer::Page(_) => {
            unreachable!("settings reads answer settings documents")
        }
    };
    let mut out = skein_lib::Queue::with_capacity(2);
    out.push(crate::Request::Now(Box::new(crate::Now::SettingsAnswer { to, key, answer: Box::new(value) })));
    out.push(crate::Request::Decided);
    Requests::Out(out)
}

pub(super) fn run_loaded(
    core: &mut Core,
    env: &Env<Limits>,
    name: skein_lib::Id<super::Pending>,
    result: super::Loaded,
) -> Requests {
    let pending = core.reads.get_mut(name).expect("pending settings load");
    let destination = core::mem::replace(&mut pending.destination, super::Destination::Done);
    let range = pending.range.clone();
    core.reads.retire(name);
    let value = match result {
        super::Loaded::Busy => Answer::None(Why::Busy),
        super::Loaded::Gone => Answer::None(Why::Gone),
        super::Loaded::Page { rows, next } => super::rows::from_store(core, &env.limits, &range, rows, next, env.wall),
        super::Loaded::TurnProblem { .. } => Answer::None(Why::Unknown),
    };
    answer(destination, value)
}

pub(super) fn setting(core: &Core, reply: ReplyTo, party: u64, setting: settings::Setting, revision: u32) -> Requests {
    if core.people.role(party, people::DEPLOYMENT).is_none() {
        return terminal(reply, Answer::None(Why::Unknown));
    }
    terminal(reply, setting_value(core, setting, revision))
}
fn setting_value(core: &Core, setting: settings::Setting, revision: u32) -> Answer {
    if !may_read(core, revision) {
        return Answer::None(Why::Unknown);
    }
    if let settings::Setting::Charter(number) = setting {
        return match core.charter_at(number, revision) {
            Some(charter) => Answer::Setting(Box::new(SettingDocument {
                setting,
                revision,
                value: settings::Value::Charter(Box::new(charter.clone())),
            })),
            None => Answer::None(Why::Unknown),
        };
    }
    if revision != core.current.newest() {
        return Answer::None(Why::Unknown);
    }
    match core.current.value(setting) {
        Some(value) => Answer::Setting(Box::new(SettingDocument { setting, revision, value: value.clone() })),
        None => Answer::None(Why::Unknown),
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Selection {
    Unknown,
    Forgotten { oldest: u32 },
    Load { first: u32, last: u32 },
}
#[must_use]
pub fn select_revision(newest: u32, oldest: u32, number: u32) -> Selection {
    if number == 0 || number > newest {
        Selection::Unknown
    } else if number < oldest {
        Selection::Forgotten { oldest }
    } else {
        Selection::Load { first: number, last: number }
    }
}
#[must_use]
pub fn page_range(newest: u32, oldest: u32, before: Option<u32>, rows: u32) -> Option<(u32, u32)> {
    let last = match before {
        Some(before) => newest.min(before.saturating_sub(1)),
        None => newest,
    };
    if last < oldest || rows == 0 {
        return None;
    }
    Some((oldest.max(last.saturating_sub(rows.saturating_sub(1))), last))
}

pub(super) fn revision(
    core: &mut Core,
    env: &Env<Limits>,
    reply: ReplyTo,
    sign_in: u64,
    party: u64,
    number: u32,
) -> Requests {
    if core.people.role(party, people::DEPLOYMENT).is_none() {
        return terminal(reply, Answer::None(Why::Unknown));
    }
    revision_read(core, env, Recipient { destination: super::Destination::Read(reply), sign_in, party }, number)
}
fn revision_read(core: &mut Core, env: &Env<Limits>, recipient: Recipient, number: u32) -> Requests {
    if core.held(number) {
        return load(core, env, recipient, number, number, true);
    }
    match select_revision(core.current.newest(), oldest(core, &env.limits), number) {
        Selection::Unknown => answer(recipient.destination, Answer::Revision(Box::new(RevisionAnswer::Unknown))),
        Selection::Forgotten { oldest } => {
            answer(recipient.destination, Answer::Revision(Box::new(RevisionAnswer::Forgotten { oldest })))
        }
        Selection::Load { first, last } => load(core, env, recipient, first, last, true),
    }
}
pub(super) fn revisions(
    core: &mut Core,
    env: &Env<Limits>,
    reply: ReplyTo,
    sign_in: u64,
    party: u64,
    before: Option<u32>,
) -> Requests {
    if core.people.role(party, people::DEPLOYMENT).is_none() {
        return terminal(reply, Answer::None(Why::Unknown));
    }
    revisions_read(core, env, Recipient { destination: super::Destination::Read(reply), sign_in, party }, before)
}
fn revisions_read(core: &mut Core, env: &Env<Limits>, recipient: Recipient, before: Option<u32>) -> Requests {
    let cutoff = oldest(core, &env.limits);
    let last = match before {
        Some(before) => core.current.newest().min(before.saturating_sub(1)),
        None => core.current.newest(),
    };
    let (oldest, before) = if last < cutoff {
        let mut held = None;
        for (&revision, _) in &core.held.heads {
            if revision <= last {
                held = Some(revision);
            }
        }
        match held {
            Some(revision) => (page_oldest(core, cutoff), revision.checked_add(1)),
            None => (cutoff, before),
        }
    } else {
        (cutoff, before)
    };
    match page_range(core.current.newest(), oldest, before, env.limits.documents.rows) {
        Some((first, last)) => load(core, env, recipient, first, last, false),
        None => answer(
            recipient.destination,
            Answer::Revisions(Box::new(RevisionsPage {
                rows: List::with_capacity(0),
                next: None,
                forgotten: oldest > 1,
            })),
        ),
    }
}
fn load(core: &mut Core, env: &Env<Limits>, recipient: Recipient, first: u32, last: u32, whole: bool) -> Requests {
    let rows = last.checked_sub(first).expect("ordered revision range").checked_add(1).expect("nonzero revision range");
    let Some(bytes) = load_bytes(&env.limits, rows) else {
        return answer(recipient.destination, Answer::None(Why::Busy));
    };
    let range = Range {
        party: recipient.party,
        scope: Scope::SettingsHistory { first, last, newest: core.current.newest(), whole },
        after: None,
        backwards: false,
        rows,
        bytes,
    };
    super::load_settings(
        core,
        env,
        super::Pending {
            destination: recipient.destination,
            sign_in: recipient.sign_in,
            party: recipient.party,
            range,
            tasks: None,
        },
    )
}
/// A selected revision is decoded whole before its values are admitted separately.
#[must_use]
pub fn load_bytes(limits: &Limits, rows: u32) -> Option<u64> {
    settings::revision_worst_case(limits)?
        .checked_add(u64::try_from(size_of::<Row>()).ok()?)?
        .checked_add(u64::try_from(size_of::<Record>()).ok()?)?
        .checked_mul(u64::from(rows))
}

/// Validate one decoded history record before it crosses a load boundary.
/// Its set values occupy at most one whole bounded settings set.
#[must_use]
pub fn valid_revision(revision: &settings::Revision, limits: &Limits) -> bool {
    let Some(capacity) = settings::capacity(limits) else { return false };
    let Some(capacity) = capacity.checked_mul(2) else { return false };
    let Ok(count) = u32::try_from(revision.items.len()) else { return false };
    if count > capacity {
        return false;
    }
    let mut seen = Map::with_capacity(count);
    let mut charters = 0_u32;
    let mut templates = 0_u32;
    let mut policies = 0_u32;
    for item in &revision.items {
        if seen.insert(item.setting, ()).ok() != Some(None) {
            return false;
        }
        match &item.edit {
            settings::Edit::Set(value) => {
                if settings::check::shape(item.setting, value, limits).is_err() {
                    return false;
                }
                match value {
                    settings::Value::Charter(value) => {
                        charters = charters.saturating_add(1);
                        if charters > limits.tasks.charters
                            || value.policy.alternatives.len()
                                > usize::try_from(limits.settings.alternatives).expect("alternative count")
                        {
                            return false;
                        }
                    }
                    settings::Value::Template(_) => {
                        templates = templates.saturating_add(1);
                        if templates > limits.templates {
                            return false;
                        }
                    }
                    settings::Value::Policy(_) => {
                        policies = policies.saturating_add(1);
                        if policies > limits.authority.projects {
                            return false;
                        }
                    }
                    settings::Value::Rules(_) => {}
                }
            }
            settings::Edit::Removed | settings::Edit::Reset => {}
        }
    }
    true
}
#[must_use]
pub fn record_revision(value: &Record) -> Option<&settings::Revision> {
    match value {
        Record::Core(CoreRecord::Settings(settings::Record::Revision { revision, .. })) => Some(revision),
        Record::Core(
            CoreRecord::Settings(
                settings::Record::Default { .. }
                | settings::Record::Changed { .. }
                | settings::Record::Charter { .. }
                | settings::Record::Proposal { .. }
                | settings::Record::Extent { .. },
            )
            | CoreRecord::Deployment(_)
            | CoreRecord::RunProof(_)
            | CoreRecord::Call(_)
            | CoreRecord::Terminal(_)
            | CoreRecord::Turn(_)
            | CoreRecord::Projection(_)
            | CoreRecord::EscalationDecision(_)
            | CoreRecord::ProposalDecision(_),
        )
        | Record::People(_)
        | Record::Tasks(_)
        | Record::Notes(_)
        | Record::Accounts(_) => None,
    }
}

fn page_oldest(core: &Core, cutoff: u32) -> u32 {
    match core.held.heads.first() {
        Some((&revision, _)) => cutoff.min(revision),
        None => cutoff,
    }
}

fn selected(scope: &Scope) -> (u32, u32, u32, bool) {
    match scope {
        Scope::SettingsHistory { first, last, newest, whole } => (*first, *last, *newest, *whole),
        Scope::Tasks { .. }
        | Scope::Task { .. }
        | Scope::Proposal { .. }
        | Scope::Escalation { .. }
        | Scope::Question { .. }
        | Scope::PersonTask { .. }
        | Scope::Result { .. }
        | Scope::Reply { .. }
        | Scope::Turn { .. }
        | Scope::EndedDelegates { .. }
        | Scope::History { .. }
        | Scope::Activity { .. }
        | Scope::Inbox { .. }
        | Scope::Notes { .. }
        | Scope::ScopedNotes { .. }
        | Scope::Spend { .. } => unreachable!("settings range dispatched by its scope"),
    }
}

pub(super) fn from_store(
    core: &Core,
    limits: &Limits,
    range: &Range,
    raw: Box<[Row]>,
    next: Option<super::Position>,
) -> Answer {
    let (first, last, newest, whole) = selected(&range.scope);
    if next.is_some() || raw.len() > usize::try_from(range.rows).expect("row cap") {
        return Answer::None(Why::Busy);
    }
    let oldest = oldest(core, limits);
    if whole && first < oldest && !core.held(first) {
        return Answer::Revision(Box::new(RevisionAnswer::Forgotten { oldest }));
    }
    let mut found = Map::with_capacity(range.rows);
    for row in raw {
        if row.owner.is_some()
            || !row.ancestors.is_empty()
            || row.related.is_some()
            || !row.linked.is_empty()
            || row.inbox.is_some()
            || row.unjoined.is_some()
            || row.ended_read.is_some()
        {
            return Answer::None(Why::Unknown);
        }
        if !super::admits(range.party, &range.scope, &row.value.key()) {
            return Answer::None(Why::Unknown);
        }
        let revision = match row.value {
            Record::Core(CoreRecord::Settings(settings::Record::Revision { revision, .. })) => revision,
            Record::Core(
                CoreRecord::Settings(
                    settings::Record::Default { .. }
                    | settings::Record::Changed { .. }
                    | settings::Record::Charter { .. }
                    | settings::Record::Proposal { .. }
                    | settings::Record::Extent { .. },
                )
                | CoreRecord::Deployment(_)
                | CoreRecord::RunProof(_)
                | CoreRecord::Call(_)
                | CoreRecord::Terminal(_)
                | CoreRecord::Turn(_)
                | CoreRecord::Projection(_)
                | CoreRecord::EscalationDecision(_)
                | CoreRecord::ProposalDecision(_),
            )
            | Record::People(_)
            | Record::Tasks(_)
            | Record::Notes(_)
            | Record::Accounts(_) => return Answer::None(Why::Unknown),
        };
        if !valid_revision(&revision, limits) {
            return Answer::None(Why::Busy);
        }
        let previous = found.insert(revision.head.number, revision).expect("bounded loaded revisions");
        if previous.is_some() {
            return Answer::None(Why::Unknown);
        }
    }
    if whole {
        return Answer::Revision(Box::new(match found.remove(&first) {
            Some(mut revision) => {
                if first == newest && newest == core.current.newest() {
                    revision.head = core.current.newest_head().clone();
                }
                RevisionAnswer::Revision { revision, value_bytes: limits.load_bytes }
            }
            None => RevisionAnswer::Forgotten { oldest },
        }));
    }
    let mut rows = List::with_capacity(range.rows);
    for number in (first..=last).rev() {
        if number < oldest && !core.held(number) {
            continue;
        }
        let Some(revision) = found.remove(&number) else { continue };
        let total = u32::try_from(revision.items.len()).expect("checked item count");
        let mut items = List::with_capacity(total.min(limits.documents.lines));
        for item in revision.items.iter().take(usize::try_from(limits.documents.lines).expect("line cap")) {
            let edit = match item.edit {
                settings::Edit::Set(_) => RevisionEdit::Set,
                settings::Edit::Removed => RevisionEdit::Removed,
                settings::Edit::Reset => RevisionEdit::Reset,
            };
            items.push(RevisionItemHead { setting: item.setting, edit }).expect("bounded item preview");
        }
        rows.push(RevisionRow { head: revision.head, more: total.saturating_sub(items.len()), items })
            .expect("bounded revision page");
    }
    let next = if first > page_oldest(core, oldest) { Some(first) } else { None };
    Answer::Revisions(Box::new(RevisionsPage { rows, next, forgotten: next.is_none() && oldest > 1 }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_lowered_history_bound_hides_rows_the_commit_could_not_erase() {
        let (mut core, mut limits) = crate::tests::core();
        core.current.head.number = 6;
        core.current.oldest = 2;
        limits.settings.history = 2;
        assert_eq!(oldest(&core, &limits), 4);
        core.current.oldest = 5;
        assert_eq!(oldest(&core, &limits), 5);
        core.current.head.number = 1;
        core.current.oldest = 0;
        assert_eq!(oldest(&core, &limits), 1);
    }
    #[test]
    fn revision_selection_names_unknown_forgotten_and_kept_ranges() {
        const NEWEST: u32 = 6;
        const OLDEST: u32 = 4;
        for (number, expected) in [
            (0, Selection::Unknown),
            (1, Selection::Forgotten { oldest: OLDEST }),
            (3, Selection::Forgotten { oldest: OLDEST }),
            (4, Selection::Load { first: 4, last: 4 }),
            (5, Selection::Load { first: 5, last: 5 }),
            (6, Selection::Load { first: 6, last: 6 }),
            (7, Selection::Unknown),
        ] {
            assert_eq!(select_revision(NEWEST, OLDEST, number), expected, "{number}");
        }
    }
    #[test]
    fn dense_pages_select_only_the_requested_predecessors() {
        for (newest, oldest, before, rows, expected) in [
            (6, 4, None, 2, Some((5, 6))),
            (6, 4, Some(5), 2, Some((4, 4))),
            (6, 1, Some(5), 2, Some((3, 4))),
            (6, 1, Some(3), 2, Some((1, 2))),
            (6, 4, None, 8, Some((4, 6))),
            (6, 4, Some(4), 2, None),
            (6, 4, Some(0), 2, None),
            (6, 4, Some(u32::MAX), 2, Some((5, 6))),
            (u32::MAX, 1, None, 2, Some((u32::MAX - 1, u32::MAX))),
        ] {
            assert_eq!(page_range(newest, oldest, before, rows), expected);
        }
    }
    #[test]
    fn loads_admit_only_the_selected_history_and_held_keys() {
        use crate::{CoreKey, Key};
        let scope = Scope::SettingsHistory { first: 4, last: 6, newest: 6, whole: false };
        for (key, expected) in [
            (settings::Key::History(3), false),
            (settings::Key::History(4), true),
            (settings::Key::History(5), true),
            (settings::Key::History(6), false),
            (settings::Key::Held(5), true),
            (settings::Key::Held(6), true),
            (settings::Key::Held(7), false),
            (settings::Key::Extent, false),
        ] {
            assert_eq!(super::super::admits(1, &scope, &Key::Core(CoreKey::Settings(key))), expected);
        }
    }
}
