//! Whole authentic Project presentation and supported settings forms; pages.md, 6.
//! Source order, independent loss and optional ledgers remain the owner's facts.
use crate::owner_project_words::ProjectPhrase as P;
use crate::phrases::jig::JigPhrase;
use crate::words::Words;
use crate::{JigBindings, JigMeaning, cards};
use skein_ui::phrases::Hole;
use skein_ui::{Builder, Role, Run};

use crate::{Borrowed, Limits, action::Act, page::project::owner_form};
use alloc::boxed::Box;
use jig_client::contract::{
    Ask, OwnerProject, OwnerProjectPolicy, OwnerProjectResourceRole, PartyKind, PolicyEdit, PolicyEditRole, PolicyGuard,
};

/// Complete standalone fragment including bindings and connector slots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProjectFragment {
    pub nodes: u32,
    pub text_bytes: u32,
    pub text_per_node: u32,
    pub depth: u32,
    pub bindings: u32,
    pub slots: u32,
}
/// The single edited channel in a whole supplied request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Spend,
    Requests,
    Decides,
    Name,
}
/// Decode the edited channel while preserving every other request value.
#[must_use]
pub fn edit(before: &Ask, field: Field, bytes: Box<[u8]>, text: u32) -> Option<Act> {
    let mut ask = before.clone();
    match &mut ask {
        Ask::ChangePolicy { edit, .. } => match edit {
            PolicyEdit::ProjectSpend { period_spend } | PolicyEdit::RoleSpend { period_spend, .. } => match field {
                Field::Spend => *period_spend = skein_ui::number(&bytes)?,
                Field::Requests | Field::Decides | Field::Name => return None,
            },
            PolicyEdit::Role(role) => match field {
                Field::Spend => role.period_spend = skein_ui::number(&bytes)?,
                Field::Requests => role.requests = u16::try_from(skein_ui::number(&bytes)?).ok()?,
                Field::Decides => role.decides = u8::try_from(skein_ui::number(&bytes)?).ok()?,
                Field::Name => return None,
            },
            PolicyEdit::Requirements { .. }
            | PolicyEdit::Projections { .. }
            | PolicyEdit::Permissions { .. }
            | PolicyEdit::Templates { .. } => {
                return None;
            }
        },
        Ask::SetPool { budget, .. } => match field {
            Field::Spend => *budget = skein_ui::number(&bytes)?,
            Field::Requests | Field::Decides | Field::Name => return None,
        },
        Ask::MakeService { name, .. } => match field {
            Field::Name => *name = jig_client::contract::Text::new(bytes, text).ok()?,
            Field::Spend | Field::Requests | Field::Decides => return None,
        },
        Ask::SetRoles { .. }
        | Ask::Message { .. }
        | Ask::Propose { .. }
        | Ask::Withdraw { .. }
        | Ask::Create { .. }
        | Ask::Compound { .. }
        | Ask::Answer { .. }
        | Ask::Take { .. }
        | Ask::HandBack { .. }
        | Ask::Decide { .. }
        | Ask::Hold { .. }
        | Ask::Lift { .. }
        | Ask::Release { .. }
        | Ask::Amend { .. }
        | Ask::Stop { .. }
        | Ask::Cancel { .. }
        | Ask::Move { .. }
        | Ask::Seen { .. }
        | Ask::EditNote { .. }
        | Ask::SignIn { .. }
        | Ask::AccountChange { .. }
        | Ask::SignOut { .. }
        | Ask::Rename { .. }
        | Ask::ChangeSettings(_)
        | Ask::MoveToNewest(_)
        | Ask::MakeProject { .. }
        | Ask::DeleteNote { .. } => return None,
    }
    Some(owner_form::action(ask))
}
/// Finite full-field envelope, derived from bounded rows and lower renderers.
#[must_use]
pub fn max_nodes(l: &Limits) -> Option<u32> {
    max_nodes_with_card(l, crate::card_nodes(l)?)
}
pub(crate) fn max_nodes_with_card(l: &Limits, card: u32) -> Option<u32> {
    let chip = crate::chip_nodes(l)?.max(crate::owner_chip::max_nodes(l)?);
    let authority = crate::owner_authority::document_nodes(l.rows, l.segments)?;
    let policy = authority
        .checked_add(6)?
        .checked_add(authority.checked_add(8)?.checked_mul(l.rows)?)?
        .checked_add(l.segments.checked_add(12)?.checked_mul(l.rows)?.checked_mul(2)?)?;
    let recurring = crate::sizes::proposed_task_nodes(l)?
        .checked_mul(l.rows)?
        .checked_add(chip)?
        .checked_add(l.rows.checked_mul(2)?)?
        .checked_add(18)?
        .checked_mul(l.rows)?;
    crate::proposed_task::part_nodes(l)?
        .checked_add(3)?
        .checked_mul(l.rows)?
        .checked_add(policy.checked_mul(3)?)?
        .checked_add(l.rows.checked_mul(34)?)?
        .checked_add(chip.checked_add(10)?.checked_mul(l.rows)?)?
        .checked_add(recurring)?
        .checked_add(l.rows.checked_mul(16)?)?
        .checked_add(l.rows.checked_add(4)?.checked_mul(l.rows.checked_add(1)?)?.checked_mul(2)?)?
        .checked_add(l.rows.checked_mul(10)?)?
        .checked_add(l.rows.checked_mul(3)?)?
        .checked_add(l.rows.checked_mul(l.rows)?.checked_mul(4)?)?
        .checked_add(l.rows.checked_mul(6)?)?
        .checked_add(crate::sizes::refusal_nodes(l)?.checked_mul(3)?)?
        .checked_add(20)?
        .checked_add(128)?
        .checked_add(historical_nodes(l, card)?)
}
// Earlier period rows include their task chips/cards and pool budgets. The
// original cut and two edge controls cost ten nodes, independently of the
// shared outer offered-action controls. Retained scalar funding adds its own
// checked fragment exactly once.
fn historical_nodes(l: &Limits, card: u32) -> Option<u32> {
    card.checked_add(crate::chip_nodes(l)?)?
        .checked_add(8)?
        .checked_mul(l.rows)?
        .checked_add(8)?
        .checked_mul(l.rows)?
        .checked_add(10)?
        .checked_add(crate::historical_pages::spend_nodes(l)?)
}
/// Every loaded saying and escaped source segment contributes to the envelope.
#[must_use]
pub fn worst_case(l: &Limits, words: &Words) -> Option<ProjectFragment> {
    let nodes = max_nodes(l)?;
    let saying = words.jig_capacity()?;
    let text_per_node = saying.max(crate::literal_segment::max_bytes(l.text)?).checked_mul(2)?;
    Some(ProjectFragment {
        nodes,
        text_bytes: nodes.checked_mul(text_per_node)?,
        text_per_node,
        depth: l.rows.checked_add(33)?,
        bindings: nodes,
        slots: nodes,
    })
}
/// Check every reserved row, raw path and nested authority/template before allocation.
#[must_use]
pub fn required(value: &OwnerProject, l: &Limits, words: &Words) -> Option<ProjectFragment> {
    if !value.name.bytes.fits(l.text)
        || value.acts.capacity() > jig_client::contract::MAX_OFFERED_ACTS
        || value.resources.capacity() > l.rows
        || value.members.capacity() > l.rows
        || value.pools.capacity() > l.rows
        || value.configured_pools.capacity() > l.rows
        || value.spend.capacity() > l.rows
        || value.recurring.capacity() > l.rows
        || value.policy.roles.capacity() > l.rows
        || value.policy.requirements.capacity() > l.rows
        || value.policy.projections.capacity() > l.rows
    {
        return None;
    }
    if let Some(permissions) = &value.policy.permissions
        && permissions.capacity() > l.rows
    {
        return None;
    }
    crate::owner_amendment::authority_fits(&value.policy.ceiling, l)?;
    for row in &value.policy.roles {
        crate::owner_amendment::authority_fits(&row.authority, l)?;
    }
    for resource in &value.resources {
        crate::owner_chip::part_fits(&resource.part, l)?;
    }
    for grant in &value.policy.projections {
        pattern_fits(&grant.pattern, l)?;
    }
    for requirement in &value.policy.requirements {
        pattern_fits(&requirement.pattern, l)?;
    }
    for member in &value.members {
        if !member.login.bytes.fits(l.text) || !member.name.bytes.fits(l.text) {
            return None;
        }
    }
    for row in &value.spend {
        chip_fits(&row.chip, l, words)?;
    }
    for row in &value.recurring {
        chip_fits(&row.chip, l, words)?;
        if row.batch.capacity() > l.rows || row.last_batch.capacity() > l.rows {
            return None;
        }
        for task in &row.batch {
            crate::owner_history::proposed_fits(task, l)?;
        }
    }
    for row in &value.acts {
        super::owner_goals::offer_fits(&row.admission, l)?;
    }
    worst_case(l, words)
}
fn pattern_fits(value: &jig_client::contract::OwnerPattern, l: &Limits) -> Option<()> {
    if value.segments.capacity() > l.segments {
        return None;
    }
    for segment in &value.segments {
        if u32::try_from(segment.len()).ok()? > l.text {
            return None;
        }
    }
    let last = match &value.last {
        jig_client::contract::OwnerPatternEnd::Exact(bytes) | jig_client::contract::OwnerPatternEnd::Open(bytes) => {
            bytes
        }
    };
    if u32::try_from(last.len()).ok()? > l.text {
        return None;
    }
    Some(())
}
fn chip_fits(value: &jig_client::contract::Chip, l: &Limits, words: &Words) -> Option<()> {
    if !value.title.bytes.fits(l.text) {
        return None;
    }
    match value.details.as_ref() {
        jig_client::contract::ChipPhase::Owner { details, .. } => {
            crate::owner_chip::required(details, l, words)?;
        }
        jig_client::contract::ChipPhase::Active(_)
        | jig_client::contract::ChipPhase::Waiting { .. }
        | jig_client::contract::ChipPhase::Idle { .. }
        | jig_client::contract::ChipPhase::Starting
        | jig_client::contract::ChipPhase::Running { .. }
        | jig_client::contract::ChipPhase::WaitingForWords { .. }
        | jig_client::contract::ChipPhase::BackingOff { .. }
        | jig_client::contract::ChipPhase::Procedure { .. }
        | jig_client::contract::ChipPhase::Person { .. }
        | jig_client::contract::ChipPhase::Closing(_)
        | jig_client::contract::ChipPhase::Held { .. }
        | jig_client::contract::ChipPhase::Ended { .. } => {}
    }
    Some(())
}
fn text(b: &mut Builder, words: &Words, phrase: P, holes: &[Hole]) {
    let text = words.book().say(words.language(), phrase.id(), holes).expect("checked project words").text;
    b.text(Run::Plain, &text);
}
fn region(b: &mut Builder, words: &Words, phrase: P, holes: &[Hole]) {
    b.open(Role::Region);
    let text = words.book().say(words.language(), phrase.id(), holes).expect("checked project region").text;
    b.name(&text);
}
fn policy(b: &mut Builder, value: &OwnerProjectPolicy, words: &Words, text_limit: u32, rows: u32) {
    region(b, words, P::Policy, &[]);
    crate::owner_authority::document_words(b, &value.ceiling, words, text_limit, rows);
    words.text(b, JigPhrase::PolicyPeriodSpend, &[Hole::Amount(value.period_spend)]);
    match value.escalation_role {
        Some(role) => text(b, words, P::EscalationRole, &[Hole::Number(u64::from(role))]),
        None => text(b, words, P::EscalationAbsent, &[]),
    }
    words.text(b, JigPhrase::PolicyProjections, &[]);
    for grant in &value.projections {
        effect(b, grant.connector, grant.kind, words);
        crate::owner_authority::pattern(b, words, &grant.pattern, text_limit);
    }
    for role in &value.roles {
        b.open(Role::Region);
        b.name(&words.say(JigPhrase::PolicyEditRole, &[Hole::Number(u64::from(role.number))]));
        words.text(b, JigPhrase::PolicyEditRole, &[Hole::Number(u64::from(role.number))]);
        crate::owner_authority::document_words(b, &role.authority, words, text_limit, rows);
        words.text(b, JigPhrase::PolicyPeriodSpend, &[Hole::Amount(role.period_spend)]);
        words.text(b, JigPhrase::PolicyRequests, &[Hole::Number(u64::from(role.requests))]);
        words.text(b, JigPhrase::PolicyDecides, &[Hole::Number(u64::from(role.decides))]);
        b.close();
    }
    words.text(b, JigPhrase::PolicyRequirements, &[]);
    for requirement in &value.requirements {
        effect(b, requirement.connector, requirement.kind, words);
        crate::owner_authority::pattern(b, words, &requirement.pattern, text_limit);
        words.text(
            b,
            JigPhrase::PolicyJudge,
            &[
                Hole::Number(u64::from(requirement.judge.connector)),
                Hole::Number(u64::from(requirement.judge.requirement)),
                Hole::Number(u64::from(requirement.judge.parameters)),
            ],
        );
        match requirement.guard {
            PolicyGuard::Guarded => words.text(b, JigPhrase::PolicyGuarded, &[]),
            PolicyGuard::Observed { freshness } => {
                words.text(b, JigPhrase::PolicyObserved, &[Hole::Number(freshness.as_nanos())]);
            }
        }
        words.text(
            b,
            if requirement.must_be_guarded { JigPhrase::PolicyMustGuard } else { JigPhrase::PolicyGuardNotRequired },
            &[],
        );
    }
    match &value.permissions {
        None => text(b, words, P::PermissionsAbsent, &[]),
        Some(mappings) => {
            crate::policy_edit::render(
                b,
                &PolicyEdit::Permissions { mappings: mappings.clone() },
                words,
                text_limit,
                rows,
            );
        }
    }
    if value.permissions_cut {
        text(b, words, P::PolicyCut, &[]);
    }
    if value.cut {
        text(b, words, P::PolicyCut, &[]);
    }
    b.close();
}
fn effect(b: &mut Builder, connector: u16, kind: u16, words: &Words) {
    words.text(b, JigPhrase::PolicyEffect, &[Hole::Number(u64::from(connector)), Hole::Number(u64::from(kind))]);
}
/// Render all source facts in order, with connector resource slots left raw.
pub fn render(b: &mut Builder, value: &OwnerProject, client: &Borrowed<'_>, words: &Words, bindings: &mut JigBindings) {
    named_region(b, words, JigPhrase::Project, &[crate::words::name(value.name.bytes())]);
    crate::document_text::loss(b, &value.name, words);
    crate::pages::work::section(
        b,
        words,
        JigPhrase::Resources,
        super::region(crate::address::ProjectSection::Resources),
    );
    resources(b, value, client, words, bindings);
    b.close();
    crate::pages::work::section(b, words, JigPhrase::Policy, super::region(crate::address::ProjectSection::Policy));
    policy_section(b, value, client, words, bindings);
    b.close();
    crate::pages::work::section(b, words, JigPhrase::Roles, super::region(crate::address::ProjectSection::Roles));
    members(b, value, client, words, bindings);
    b.close();
    crate::pages::work::section(b, words, JigPhrase::Spend, super::region(crate::address::ProjectSection::Spend));
    ledgers(b, value, client, words, bindings);
    configured_ledgers(b, value, words);
    spend(b, value, client, words, bindings);
    b.close();
    crate::pages::work::section(
        b,
        words,
        JigPhrase::Recurring,
        super::region(crate::address::ProjectSection::Recurring),
    );
    recurring(b, value, client, words, bindings);
    b.close();
    if value.cut {
        text(b, words, P::ProjectCut, &[]);
    }
    b.close();
}
fn configured_ledgers(b: &mut Builder, value: &OwnerProject, words: &Words) {
    region(b, words, P::ConfiguredPeriod, &[Hole::Number(value.configured_period)]);
    text(b, words, P::ConfiguredPeriod, &[Hole::Number(value.configured_period)]);
    for pool in &value.configured_pools {
        crate::owner_budget::ledger(b, pool, words);
    }
    if value.configured_pools_cut {
        text(b, words, P::ProjectCut, &[]);
    }
    b.close();
}
fn resources(b: &mut Builder, value: &OwnerProject, _client: &Borrowed<'_>, words: &Words, bindings: &mut JigBindings) {
    b.open(Role::List);
    for resource in &value.resources {
        b.open(Role::Item);
        text(b, words, P::Resource, &[]);
        text(
            b,
            words,
            match resource.role {
                OwnerProjectResourceRole::Owned => P::Owned,
                OwnerProjectResourceRole::Participant => P::Participant,
                OwnerProjectResourceRole::Unavailable => P::Unavailable,
                OwnerProjectResourceRole::Context => P::Context,
            },
            &[],
        );
        crate::proposed_task::part(b, &resource.part, words, bindings);
        b.close();
    }
    b.close();
}
fn policy_section(
    b: &mut Builder,
    value: &OwnerProject,
    client: &Borrowed<'_>,
    words: &Words,
    bindings: &mut JigBindings,
) {
    let text_limit = bindings.text_limit();
    policy(b, &value.policy, words, text_limit, client.limits.rows);
}
fn members(b: &mut Builder, value: &OwnerProject, _client: &Borrowed<'_>, words: &Words, _bindings: &mut JigBindings) {
    b.open(Role::List);
    for member in &value.members {
        b.open(Role::Item);
        region(b, words, P::Member, &[Hole::Number(member.party)]);
        text(
            b,
            words,
            match member.kind {
                PartyKind::Person => P::Person,
                PartyKind::Service => P::Service,
            },
            &[],
        );
        words.held_role(b, Some(member.role));
        text(b, words, P::Login, &[]);
        crate::literal_segment::render(b, member.login.bytes());
        crate::document_text::loss(b, &member.login, words);
        text(b, words, P::Name, &[]);
        crate::literal_segment::render(b, member.name.bytes());
        crate::document_text::loss(b, &member.name, words);
        b.close();
        b.close();
    }
    b.close();
}
fn ledgers(b: &mut Builder, value: &OwnerProject, _client: &Borrowed<'_>, words: &Words, _bindings: &mut JigBindings) {
    match &value.current_period {
        Some(ledger) => crate::owner_budget::ledger(b, ledger, words),
        None => text(b, words, P::PeriodAbsent, &[]),
    }
    for ledger in &value.pools {
        crate::owner_budget::ledger(b, ledger, words);
    }
}
fn spend(b: &mut Builder, value: &OwnerProject, client: &Borrowed<'_>, words: &Words, bindings: &mut JigBindings) {
    b.open(Role::List);
    for spend in &value.spend {
        b.open(Role::Item);
        region(b, words, P::Spend, &[]);
        cards::chip::chip(b, client, &spend.chip, words, bindings);
        crate::proposed_task::funder(b, spend.funder, words);
        text(b, words, P::Budget, &[Hole::Amount(spend.numbers.budget)]);
        text(b, words, P::Spent, &[Hole::Amount(spend.numbers.spent)]);
        text(b, words, P::Below, &[Hole::Amount(spend.numbers.spent_below)]);
        text(b, words, P::Reserved, &[Hole::Amount(spend.numbers.reserved)]);
        b.close();
        b.close();
    }
    b.close();
}
fn recurring(b: &mut Builder, value: &OwnerProject, client: &Borrowed<'_>, words: &Words, bindings: &mut JigBindings) {
    for row in &value.recurring {
        whole_recurring(b, client, row, words, bindings);
    }
}
// A new whole owner row uses its supplied numeric identity even with an empty title.
fn whole_recurring(
    b: &mut Builder,
    client: &Borrowed<'_>,
    row: &jig_client::contract::Recurring,
    words: &Words,
    bindings: &mut JigBindings,
) {
    named_region(b, words, JigPhrase::RecurringKey, &[Hole::Number(u64::from(row.key))]);
    cards::chip::chip(b, client, &row.chip, words, bindings);
    words.text(b, JigPhrase::RecurringKey, &[Hole::Number(u64::from(row.key))]);
    b.open(Role::List);
    for task in &row.batch {
        cards::proposal::proposed_task(b, task, words, bindings, client.limits.rows);
    }
    b.close();
    words.text(b, JigPhrase::Budget, &[Hole::Amount(row.budget)]);
    words.text(
        b,
        match row.overlap {
            jig_client::contract::RecurringOverlap::Skip => JigPhrase::RecurringSkip,
            jig_client::contract::RecurringOverlap::Wait => JigPhrase::RecurringWait,
        },
        &[],
    );
    words.text(b, JigPhrase::CurrentPeriod, &[Hole::Number(row.current_period)]);
    if let Some(began) = row.began {
        b.open(Role::Time);
        words.text(b, JigPhrase::PeriodBegan, &[Hole::Wall(words.adjusted_wall(began))]);
        b.close();
    }
    match row.next {
        Some(next) => {
            b.open(Role::Time);
            words.text(b, JigPhrase::NextRun, &[Hole::Wall(words.adjusted_wall(next))]);
            b.close();
        }
        None => words.text(b, JigPhrase::NextPeriod, &[]),
    }
    words.text(b, JigPhrase::RecurringLastPeriod, &[Hole::Number(row.last_period)]);
    if let Some(period) = row.pending_period {
        words.text(b, JigPhrase::RecurringPendingPeriod, &[Hole::Number(period)]);
    }
    for task in &row.last_batch {
        crate::frame::control(
            b,
            words,
            bindings,
            Role::Link,
            JigPhrase::TaskPage,
            &[Hole::Number(*task)],
            Act::Go { address: crate::address::Address::task(*task) },
        );
    }
    if row.cut {
        words.status(b, JigPhrase::RecurringCut, &[]);
    }
    b.close();
}
/// Compose the selected genuine Project section from supplied facts.
pub(super) fn section(
    b: &mut Builder,
    value: &OwnerProject,
    selected: crate::address::ProjectSection,
    client: &Borrowed<'_>,
    words: &Words,
    bindings: &mut JigBindings,
) {
    use crate::address::ProjectSection;
    let project_page = client.project_page();
    match selected {
        ProjectSection::Settings => {
            render(b, value, client, words, bindings);
            forms(b, value, client, words, bindings);
            super::historical_spend(b, client, words, bindings);
        }
        ProjectSection::Resources => resources(b, value, client, words, bindings),
        ProjectSection::Policy => {
            policy_section(b, value, client, words, bindings);
            forms(b, value, client, words, bindings);
        }
        ProjectSection::Roles => members(b, value, client, words, bindings),
        ProjectSection::Spend => {
            ledgers(b, value, client, words, bindings);
            spend(b, value, client, words, bindings);
            super::historical_spend(b, client, words, bindings);
        }
        ProjectSection::Recurring => recurring(b, value, client, words, bindings),
        ProjectSection::Overview => {
            super::inbox_cards(b, client, words, bindings);
            for goal in &project_page.goals {
                super::goals::line(b, client, goal, words, bindings);
            }
            ledgers(b, value, client, words, bindings);
        }
        ProjectSection::Goals => {
            if !super::goals::available(b, client, value.number, words) {
                return;
            }
            words.text(b, JigPhrase::PriorityMeaning, &[]);
            b.open(Role::List);
            for goal in &project_page.listed_goals {
                super::goals::line(b, client, goal, words, bindings);
            }
            b.close();
            if project_page.goals_more {
                words.status(b, JigPhrase::MoreRows, &[]);
            }
            crate::pages::cut(b, words, project_page.ended_cut);
            crate::pages::work::cursors(
                b,
                words,
                bindings,
                project_page.ended_before.as_ref(),
                project_page.ended_after.as_ref(),
                crate::page::project::GOALS_WINDOW,
            );
            super::owner_goals::form(b, client, value, words, bindings);
        }
        ProjectSection::Tasks => {
            b.open(Role::List);
            for row in &project_page.tasks {
                b.open(Role::Item);
                cards::chip::chip(b, client, &row.chip, words, bindings);
                crate::owner_result::requester(b, row.requester, words);
                words.wall(b, row.changed);
                b.close();
            }
            b.close();
            if project_page.tasks_more {
                words.status(b, JigPhrase::MoreRows, &[]);
            }
        }
        ProjectSection::Notes => super::notes::notes(b, client, value.number, words, bindings),
    }
}
fn request_field(b: &mut Builder, ask: &Ask, field: Field, value: u64, words: &Words, bindings: &mut JigBindings) {
    b.open(Role::TextField);
    let phrase = match field {
        Field::Spend => JigPhrase::PolicyPeriodSpend,
        Field::Requests => JigPhrase::PolicyRequests,
        Field::Decides => JigPhrase::PolicyDecides,
        Field::Name => return,
    };
    let hole = match field {
        Field::Spend => Hole::Amount(value),
        Field::Requests | Field::Decides => Hole::Number(value),
        Field::Name => return,
    };
    b.name(&words.say(phrase, &[hole]));
    let decimal = skein_lib::Decimal::of(value);
    b.value(decimal.as_bytes(), 0);
    b.bind(bindings.insert(JigMeaning::ProjectSetting { ask: Box::new(ask.clone()), field }));
    b.close();
}
fn control(b: &mut Builder, ask: &Ask, words: &Words, bindings: &mut JigBindings) {
    b.open(Role::Button);
    let label = words.book().say(words.language(), P::Edit.id(), &[]).expect("project edit label").text;
    b.name(&label);
    b.bind(bindings.insert(JigMeaning::Action(owner_form::action(ask.clone()))));
    b.close();
}
// Extract only complete typed source requests with exhaustive contract matches.
fn policy_request(ask: &Ask) -> Option<&PolicyEdit> {
    match ask {
        Ask::ChangePolicy { edit, .. } => Some(edit),
        Ask::Message { .. }
        | Ask::Propose { .. }
        | Ask::Withdraw { .. }
        | Ask::Create { .. }
        | Ask::Compound { .. }
        | Ask::Answer { .. }
        | Ask::Take { .. }
        | Ask::HandBack { .. }
        | Ask::Decide { .. }
        | Ask::Hold { .. }
        | Ask::Lift { .. }
        | Ask::Release { .. }
        | Ask::Amend { .. }
        | Ask::Stop { .. }
        | Ask::Cancel { .. }
        | Ask::Move { .. }
        | Ask::Seen { .. }
        | Ask::EditNote { .. }
        | Ask::SignIn { .. }
        | Ask::AccountChange { .. }
        | Ask::SignOut { .. }
        | Ask::Rename { .. }
        | Ask::ChangeSettings(_)
        | Ask::MoveToNewest(_)
        | Ask::MakeProject { .. }
        | Ask::DeleteNote { .. }
        | Ask::SetRoles { .. }
        | Ask::MakeService { .. }
        | Ask::SetPool { .. } => None,
    }
}
fn role_request(ask: &Ask) -> Option<&PolicyEditRole> {
    match policy_request(ask)? {
        PolicyEdit::Role(role) => Some(role),
        PolicyEdit::RoleSpend { .. }
        | PolicyEdit::ProjectSpend { .. }
        | PolicyEdit::Requirements { .. }
        | PolicyEdit::Projections { .. }
        | PolicyEdit::Permissions { .. }
        | PolicyEdit::Templates { .. } => None,
    }
}
fn spend_request(ask: &Ask) -> Option<u64> {
    match policy_request(ask)? {
        PolicyEdit::ProjectSpend { period_spend } => Some(*period_spend),
        PolicyEdit::RoleSpend { .. }
        | PolicyEdit::Role(_)
        | PolicyEdit::Requirements { .. }
        | PolicyEdit::Projections { .. }
        | PolicyEdit::Permissions { .. }
        | PolicyEdit::Templates { .. } => None,
    }
}
fn requirements_request(ask: &Ask) -> Option<&skein_lib::List<jig_client::contract::PolicyRequirement>> {
    match policy_request(ask)? {
        PolicyEdit::Requirements { requirements } => Some(requirements),
        PolicyEdit::RoleSpend { .. }
        | PolicyEdit::ProjectSpend { .. }
        | PolicyEdit::Role(_)
        | PolicyEdit::Projections { .. }
        | PolicyEdit::Permissions { .. }
        | PolicyEdit::Templates { .. } => None,
    }
}
fn projections_request(ask: &Ask) -> Option<&skein_lib::List<jig_client::contract::OwnerGrant>> {
    match policy_request(ask)? {
        PolicyEdit::Projections { grants } => Some(grants),
        PolicyEdit::RoleSpend { .. }
        | PolicyEdit::ProjectSpend { .. }
        | PolicyEdit::Role(_)
        | PolicyEdit::Requirements { .. }
        | PolicyEdit::Permissions { .. }
        | PolicyEdit::Templates { .. } => None,
    }
}
fn permissions_request(ask: &Ask) -> Option<&skein_lib::List<jig_client::contract::PolicyPermission>> {
    match policy_request(ask)? {
        PolicyEdit::Permissions { mappings } => Some(mappings),
        PolicyEdit::RoleSpend { .. }
        | PolicyEdit::ProjectSpend { .. }
        | PolicyEdit::Role(_)
        | PolicyEdit::Requirements { .. }
        | PolicyEdit::Projections { .. }
        | PolicyEdit::Templates { .. } => None,
    }
}
fn holdings_request(ask: &Ask) -> Option<&skein_lib::List<jig_client::contract::Holding>> {
    match ask {
        Ask::SetRoles { holdings, .. } => Some(holdings),
        Ask::Message { .. }
        | Ask::Propose { .. }
        | Ask::Withdraw { .. }
        | Ask::Create { .. }
        | Ask::Compound { .. }
        | Ask::Answer { .. }
        | Ask::Take { .. }
        | Ask::HandBack { .. }
        | Ask::Decide { .. }
        | Ask::Hold { .. }
        | Ask::Lift { .. }
        | Ask::Release { .. }
        | Ask::Amend { .. }
        | Ask::Stop { .. }
        | Ask::Cancel { .. }
        | Ask::Move { .. }
        | Ask::Seen { .. }
        | Ask::EditNote { .. }
        | Ask::SignIn { .. }
        | Ask::AccountChange { .. }
        | Ask::SignOut { .. }
        | Ask::Rename { .. }
        | Ask::ChangeSettings(_)
        | Ask::MoveToNewest(_)
        | Ask::MakeProject { .. }
        | Ask::DeleteNote { .. }
        | Ask::ChangePolicy { .. }
        | Ask::MakeService { .. }
        | Ask::SetPool { .. } => None,
    }
}
fn pool_request(ask: &Ask) -> bool {
    match ask {
        Ask::SetPool { .. } => true,
        Ask::Message { .. }
        | Ask::Propose { .. }
        | Ask::Withdraw { .. }
        | Ask::Create { .. }
        | Ask::Compound { .. }
        | Ask::Answer { .. }
        | Ask::Take { .. }
        | Ask::HandBack { .. }
        | Ask::Decide { .. }
        | Ask::Hold { .. }
        | Ask::Lift { .. }
        | Ask::Release { .. }
        | Ask::Amend { .. }
        | Ask::Stop { .. }
        | Ask::Cancel { .. }
        | Ask::Move { .. }
        | Ask::Seen { .. }
        | Ask::EditNote { .. }
        | Ask::SignIn { .. }
        | Ask::AccountChange { .. }
        | Ask::SignOut { .. }
        | Ask::Rename { .. }
        | Ask::ChangeSettings(_)
        | Ask::MoveToNewest(_)
        | Ask::MakeProject { .. }
        | Ask::DeleteNote { .. }
        | Ask::ChangePolicy { .. }
        | Ask::SetRoles { .. }
        | Ask::MakeService { .. } => false,
    }
}
fn service_request(ask: &Ask) -> Option<(&jig_client::contract::Text, jig_client::contract::HeldRole)> {
    match ask {
        Ask::MakeService { name, role, .. } => Some((name, *role)),
        Ask::Message { .. }
        | Ask::Propose { .. }
        | Ask::Withdraw { .. }
        | Ask::Create { .. }
        | Ask::Compound { .. }
        | Ask::Answer { .. }
        | Ask::Take { .. }
        | Ask::HandBack { .. }
        | Ask::Decide { .. }
        | Ask::Hold { .. }
        | Ask::Lift { .. }
        | Ask::Release { .. }
        | Ask::Amend { .. }
        | Ask::Stop { .. }
        | Ask::Cancel { .. }
        | Ask::Move { .. }
        | Ask::Seen { .. }
        | Ask::EditNote { .. }
        | Ask::SignIn { .. }
        | Ask::AccountChange { .. }
        | Ask::SignOut { .. }
        | Ask::Rename { .. }
        | Ask::ChangeSettings(_)
        | Ask::MoveToNewest(_)
        | Ask::MakeProject { .. }
        | Ask::DeleteNote { .. }
        | Ask::ChangePolicy { .. }
        | Ask::SetRoles { .. }
        | Ask::SetPool { .. } => None,
    }
}
/// Show only complete supplied policy replacements, never absent mappings.
pub(super) fn forms(
    b: &mut Builder,
    value: &OwnerProject,
    client: &Borrowed<'_>,
    words: &Words,
    bindings: &mut JigBindings,
) {
    let state = &client.project_page().settings;
    policy_fields(b, value, state, words, bindings);
    policy_lists(b, value, state, words, bindings);
    pool_forms(b, value, state, words, bindings);
    holding_forms(b, value, state, words, bindings);
    service_form(b, value, state, words, bindings);
    review_form(b, client, state, words, bindings);
}
fn policy_fields(
    b: &mut Builder,
    value: &OwnerProject,
    state: &owner_form::Form,
    words: &Words,
    bindings: &mut JigBindings,
) {
    let spend = Ask::ChangePolicy {
        project: value.number,
        edit: PolicyEdit::ProjectSpend { period_spend: value.policy.period_spend },
    };
    if owner_form::supported(value, &spend) {
        named_region(b, words, JigPhrase::PolicyPeriodSpend, &[Hole::Amount(value.policy.period_spend)]);
        let spend = owner_form::draft(state, &spend);
        if let Some(period_spend) = spend_request(&spend) {
            request_field(b, &spend, Field::Spend, period_spend, words, bindings);
        }
        control(b, &spend, words, bindings);
        b.close();
    }
    for role in &value.policy.roles {
        if let Some(authority) = role.authority.whole() {
            let ask = Ask::ChangePolicy {
                project: value.number,
                edit: PolicyEdit::Role(PolicyEditRole {
                    number: role.number,
                    authority,
                    period_spend: role.period_spend,
                    requests: role.requests,
                    decides: role.decides,
                }),
            };
            if owner_form::supported(value, &ask) {
                named_region(b, words, JigPhrase::PolicyEditRole, &[Hole::Number(u64::from(role.number))]);
                let ask = owner_form::draft(state, &ask);
                if let Some(chosen) = role_request(&ask) {
                    request_field(b, &ask, Field::Spend, chosen.period_spend, words, bindings);
                    request_field(b, &ask, Field::Requests, u64::from(chosen.requests), words, bindings);
                    request_field(b, &ask, Field::Decides, u64::from(chosen.decides), words, bindings);
                }
                control(b, &ask, words, bindings);
                b.close();
            }
        }
    }
}
fn policy_lists(
    b: &mut Builder,
    value: &OwnerProject,
    state: &owner_form::Form,
    words: &Words,
    bindings: &mut JigBindings,
) {
    let requirements = Ask::ChangePolicy {
        project: value.number,
        edit: PolicyEdit::Requirements { requirements: value.policy.requirements.clone() },
    };
    if owner_form::supported(value, &requirements) {
        named_region(b, words, JigPhrase::PolicyRequirements, &[]);
        let draft = owner_form::draft(state, &requirements);
        if let Some(requirements) = requirements_request(&draft) {
            for remove in 0..requirements.len() {
                let mut remaining = skein_lib::List::with_capacity(requirements.capacity());
                for index in 0..requirements.len() {
                    if index != remove {
                        remaining
                            .push(requirements.get(index).expect("bounded requirement").clone())
                            .expect("whole source capacity");
                    }
                }
                remove_control(
                    b,
                    &Ask::ChangePolicy {
                        project: value.number,
                        edit: PolicyEdit::Requirements { requirements: remaining },
                    },
                    remove,
                    words,
                    bindings,
                );
            }
        }
        b.close();
    }
    let projections = Ask::ChangePolicy {
        project: value.number,
        edit: PolicyEdit::Projections { grants: value.policy.projections.clone() },
    };
    if owner_form::supported(value, &projections) {
        named_region(b, words, JigPhrase::PolicyProjections, &[]);
        let draft = owner_form::draft(state, &projections);
        if let Some(grants) = projections_request(&draft) {
            for remove in 0..grants.len() {
                let mut remaining = skein_lib::List::with_capacity(grants.capacity());
                for index in 0..grants.len() {
                    if index != remove {
                        remaining
                            .push(grants.get(index).expect("bounded projection").clone())
                            .expect("whole source capacity");
                    }
                }
                remove_control(
                    b,
                    &Ask::ChangePolicy { project: value.number, edit: PolicyEdit::Projections { grants: remaining } },
                    remove,
                    words,
                    bindings,
                );
            }
        }
        b.close();
    }
    if let Some(mappings) = &value.policy.permissions {
        let before =
            Ask::ChangePolicy { project: value.number, edit: PolicyEdit::Permissions { mappings: mappings.clone() } };
        if owner_form::supported(value, &before) {
            named_region(b, words, JigPhrase::PolicyPermissions, &[]);
            let draft = owner_form::draft(state, &before);
            if let Some(mappings) = permissions_request(&draft) {
                for remove in 0..mappings.len() {
                    let mut remaining = skein_lib::List::with_capacity(mappings.capacity());
                    for index in 0..mappings.len() {
                        if index != remove {
                            remaining
                                .push(*mappings.get(index).expect("bounded permission"))
                                .expect("whole mapping capacity");
                        }
                    }
                    remove_control(
                        b,
                        &Ask::ChangePolicy {
                            project: value.number,
                            edit: PolicyEdit::Permissions { mappings: remaining },
                        },
                        remove,
                        words,
                        bindings,
                    );
                }
                // Even a supplied empty list remains an explicit complete baseline.
                control(b, &draft, words, bindings);
            }
            b.close();
        }
    }
}
fn holding_forms(
    b: &mut Builder,
    value: &OwnerProject,
    state: &owner_form::Form,
    words: &Words,
    bindings: &mut JigBindings,
) {
    let mut holdings = skein_lib::List::with_capacity(value.members.capacity());
    for member in &value.members {
        holdings
            .push(jig_client::contract::Holding { party: member.party, role: member.role })
            .expect("whole member capacity");
    }
    let roles = Ask::SetRoles { project: value.number, holdings };
    if owner_form::supported(value, &roles) {
        let draft = owner_form::draft(state, &roles);
        if let Some(holdings) = holdings_request(&draft) {
            for remove in 0..holdings.len() {
                let member = holdings.get(remove).expect("bounded holding");
                region(b, words, P::Member, &[Hole::Number(member.party)]);
                for role in [
                    jig_client::contract::HeldRole::Owner,
                    jig_client::contract::HeldRole::Maintainer,
                    jig_client::contract::HeldRole::Member,
                    jig_client::contract::HeldRole::Observer,
                ] {
                    holding_control(b, value.number, holdings, remove, role, words, bindings);
                }
                for role in &value.policy.roles {
                    holding_control(
                        b,
                        value.number,
                        holdings,
                        remove,
                        jig_client::contract::HeldRole::Policy { role: role.number },
                        words,
                        bindings,
                    );
                }
                let mut remaining = skein_lib::List::with_capacity(holdings.capacity());
                for index in 0..holdings.len() {
                    if index != remove {
                        remaining.push(*holdings.get(index).expect("bounded holding")).expect("whole holding capacity");
                    }
                }
                remove_control(
                    b,
                    &Ask::SetRoles { project: value.number, holdings: remaining },
                    remove,
                    words,
                    bindings,
                );
                b.close();
            }
        }
    }
}
fn review_form(
    b: &mut Builder,
    client: &Borrowed<'_>,
    state: &owner_form::Form,
    words: &Words,
    bindings: &mut JigBindings,
) {
    let text_limit = bindings.text_limit();
    if let Some(ask) = &state.review {
        region(b, words, P::Review, &[]);
        words.text(b, JigPhrase::Before, &[]);
        if let Some(before) = &state.before {
            render_request(b, before, words, text_limit, client.limits.rows);
        } else if pool_request(ask) {
            text(b, words, P::PoolAbsent, &[]);
        }
        if let Some(period) = state.pool_period {
            text(b, words, P::ConfiguredPeriod, &[Hole::Number(period)]);
        }
        words.text(b, JigPhrase::After, &[]);
        render_request(b, ask, words, text_limit, client.limits.rows);
        crate::frame::control(b, words, bindings, Role::Button, JigPhrase::Confirm, &[], Act::Confirm);
        crate::frame::control(b, words, bindings, Role::Button, JigPhrase::Dismiss, &[], Act::Dismiss);
        b.close();
    }
    if let Some(refused) = &state.refused {
        words.refusal(b, refused, text_limit);
    }
    if let Some(request) = state.pending
        && let Some(intent) = client.intents().get(skein_lib::Id::from_token(request))
    {
        words.progress(b, intent.progress);
    }
}

fn pool_forms(
    b: &mut Builder,
    value: &OwnerProject,
    state: &owner_form::Form,
    words: &Words,
    bindings: &mut JigBindings,
) {
    for member in &value.members {
        let mut budget = None;
        for pool in &value.configured_pools {
            match pool.source {
                jig_client::contract::Funder::Pool { project, person, period }
                    if project == value.number && person == member.party && period == value.configured_period =>
                {
                    budget = Some(pool.numbers.budget);
                    break;
                }
                jig_client::contract::Funder::Pool { .. }
                | jig_client::contract::Funder::Task(_)
                | jig_client::contract::Funder::Period { .. }
                | jig_client::contract::Funder::Recurring { .. } => {}
            }
        }
        let mut offered = false;
        for offer in &value.acts {
            if offer.act == jig_client::contract::ActKind::SetPool {
                offered = match &offer.admission {
                    jig_client::contract::Admission::Allowed | jig_client::contract::Admission::Proposed { .. } => true,
                    jig_client::contract::Admission::Refused(_) | jig_client::contract::Admission::Bounded { .. } => {
                        false
                    }
                };
                break;
            }
        }
        if !offered || value.configured_pools_cut {
            continue;
        }
        region(b, words, P::Member, &[Hole::Number(member.party)]);
        text(b, words, P::ConfiguredPeriod, &[Hole::Number(value.configured_period)]);
        if budget.is_none() {
            text(b, words, P::PoolAbsent, &[]);
        }
        b.open(Role::TextField);
        b.name(&words.say(JigPhrase::GoalBudget, &[]));
        let reviewed = match state.review.as_deref() {
            Some(Ask::SetPool { project, party, budget }) if *project == value.number && *party == member.party => {
                Some(*budget)
            }
            Some(_) | None => budget,
        };
        if let Some(budget) = reviewed {
            let digits = skein_lib::Decimal::of(budget);
            b.value(digits.as_bytes(), 0);
        }
        b.bind(bindings.insert(JigMeaning::ProjectPoolBudget { project: value.number, party: member.party }));
        b.close();
        b.close();
    }
}
fn named_region(b: &mut Builder, words: &Words, phrase: JigPhrase, holes: &[Hole]) {
    b.open(Role::Region);
    b.name(&words.say(phrase, holes));
}
fn role_label(words: &Words, role: jig_client::contract::HeldRole) -> Box<[u8]> {
    match role {
        jig_client::contract::HeldRole::Owner => words.say(JigPhrase::HeldOwner, &[]),
        jig_client::contract::HeldRole::Maintainer => words.say(JigPhrase::HeldMaintainer, &[]),
        jig_client::contract::HeldRole::Member => words.say(JigPhrase::HeldMember, &[]),
        jig_client::contract::HeldRole::Observer => words.say(JigPhrase::HeldObserver, &[]),
        jig_client::contract::HeldRole::Policy { role } => {
            words.say(JigPhrase::HeldPolicy, &[Hole::Number(u64::from(role))])
        }
    }
}
fn role_control(
    b: &mut Builder,
    role: jig_client::contract::HeldRole,
    action: Act,
    words: &Words,
    bindings: &mut JigBindings,
) {
    b.open(Role::Button);
    let label = role_label(words, role);
    b.name(&label);
    b.bind(bindings.insert(JigMeaning::Action(action)));
    b.close();
}
fn holding_control(
    b: &mut Builder,
    project: u32,
    source: &skein_lib::List<jig_client::contract::Holding>,
    index: u32,
    role: jig_client::contract::HeldRole,
    words: &Words,
    bindings: &mut JigBindings,
) {
    let mut holdings = source.clone();
    holdings.get_mut(index).expect("bounded selected holding").role = role;
    role_control(b, role, owner_form::action(Ask::SetRoles { project, holdings }), words, bindings);
}
fn service_form(
    b: &mut Builder,
    value: &OwnerProject,
    state: &owner_form::Form,
    words: &Words,
    bindings: &mut JigBindings,
) {
    if !owner_form::service_offered(value) {
        return;
    }
    named_region(b, words, JigPhrase::ActMakeService, &[]);
    for role in [
        jig_client::contract::HeldRole::Owner,
        jig_client::contract::HeldRole::Maintainer,
        jig_client::contract::HeldRole::Member,
        jig_client::contract::HeldRole::Observer,
    ] {
        role_control(b, role, Act::ProjectServiceRole { role }, words, bindings);
    }
    for row in &value.policy.roles {
        let role = jig_client::contract::HeldRole::Policy { role: row.number };
        role_control(b, role, Act::ProjectServiceRole { role }, words, bindings);
    }
    if let Some(role) = state.service_role {
        words.held_role(b, Some(role));
        b.open(Role::TextField);
        let name = words.book().say(words.language(), P::Name.id(), &[]).expect("checked service name").text;
        b.name(&name);
        if let Some(ask) = &state.review
            && let Some((name, chosen)) = service_request(ask)
            && chosen == role
        {
            b.value(name.bytes(), 0);
        }
        b.bind(bindings.insert(JigMeaning::ProjectServiceName { project: value.number, role }));
        b.close();
    }
    b.close();
}
fn remove_control(b: &mut Builder, ask: &Ask, index: u32, words: &Words, bindings: &mut JigBindings) {
    b.open(Role::Button);
    let label = words
        .book()
        .say(words.language(), P::Remove.id(), &[Hole::Number(u64::from(index))])
        .expect("checked remove")
        .text;
    b.name(&label);
    b.bind(bindings.insert(JigMeaning::Action(owner_form::action(ask.clone()))));
    b.close();
}
fn render_request(b: &mut Builder, ask: &Ask, words: &Words, text_limit: u32, rows: u32) {
    match ask {
        Ask::ChangePolicy { edit, .. } => crate::policy_edit::render(b, edit, words, text_limit, rows),
        Ask::SetRoles { holdings, .. } => {
            for row in holdings {
                words.actor(b, jig_client::contract::Actor::Party(row.party));
                words.held_role(b, Some(row.role));
            }
        }
        Ask::SetPool { party, budget, .. } => {
            words.actor(b, jig_client::contract::Actor::Party(*party));
            words.amount(b, *budget);
        }
        Ask::MakeService { name, role, .. } => {
            b.text(Run::Plain, name.bytes());
            words.held_role(b, Some(*role));
        }
        Ask::Message { .. }
        | Ask::Propose { .. }
        | Ask::Withdraw { .. }
        | Ask::Create { .. }
        | Ask::Compound { .. }
        | Ask::Answer { .. }
        | Ask::Take { .. }
        | Ask::HandBack { .. }
        | Ask::Decide { .. }
        | Ask::Hold { .. }
        | Ask::Lift { .. }
        | Ask::Release { .. }
        | Ask::Amend { .. }
        | Ask::Stop { .. }
        | Ask::Cancel { .. }
        | Ask::Move { .. }
        | Ask::Seen { .. }
        | Ask::EditNote { .. }
        | Ask::SignIn { .. }
        | Ask::AccountChange { .. }
        | Ask::SignOut { .. }
        | Ask::Rename { .. }
        | Ask::ChangeSettings(_)
        | Ask::MoveToNewest(_)
        | Ask::MakeProject { .. }
        | Ask::DeleteNote { .. } => {}
    }
}
