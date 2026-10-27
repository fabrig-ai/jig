//! Independent whole settings fixtures (client/domain/contract.md, 3–7).
use jig_client::contract::{
    AuthorityModels, ChangeSettings, Charter, CharterDocument, DocumentText, EndedBy, HeldRevision, LlmChoice,
    ModelEffort, OwnerAuthority, OwnerDelegation, OwnerGrant, OwnerKindImplication, OwnerPattern, OwnerPatternEnd,
    PolicyEditRole, PolicyGuard, PolicyJudge, PolicyPermission, PolicyRequirement, ProposalKey, Proposer, Retention,
    RevisionDecider, RevisionDocument, RevisionEdit, RevisionHead, RevisionItem, RevisionItemEdit, RevisionItemHead,
    RevisionParty, RevisionRow, RevisionsPage, Rules, Running, Setting, SettingEdit, SettingFollows, SettingHosts,
    SettingItem, SettingLine, SettingModel, SettingNumber, SettingPolicy, SettingSet, SettingValue, SettingsDocument,
    Size, TaskContractDocument, TaskFlags, Template, TemplateBudget, TemplateDeadline, TemplateExecutor,
    TemplateOverride, Text,
};
use skein_lib::{Duration, List, Wall};
fn text(value: &[u8]) -> Text {
    Text::new(value.into(), 4096).expect("fixture UTF-8")
}
fn pattern() -> OwnerPattern {
    OwnerPattern { segments: List::with_capacity(0), last: OwnerPatternEnd::Open(b"a".as_slice().into()) }
}
#[must_use]
pub fn settings_authority() -> OwnerAuthority {
    OwnerAuthority {
        models: Some(AuthorityModels::All),
        tools: 15,
        grants: List::with_capacity(0),
        delegation: OwnerDelegation { kinds: List::with_capacity(0), tasks: 9, depth: 3 },
        spend: 101,
        deadline: Some(Wall::from_nanos(7)),
        notes: 3,
        note_resources: List::with_capacity(0),
    }
}
fn requirement() -> PolicyRequirement {
    PolicyRequirement {
        connector: 2,
        kind: 3,
        pattern: pattern(),
        judge: PolicyJudge { connector: 4, requirement: 5, parameters: 6 },
        guard: PolicyGuard::Observed { freshness: Duration::from_secs(7) },
        must_be_guarded: true,
    }
}
fn model() -> SettingModel {
    let mut efforts = List::with_capacity(1);
    efforts.push(text(b"high")).expect("one effort");
    SettingModel {
        dialect: 1,
        endpoint: 2,
        program: Some(text(b"runner")),
        name: text(b"model"),
        window: 10000,
        output: 100,
        input_price: 3,
        cached_price: 4,
        cache_write_price: Some(5),
        output_price: 6,
        price_unit: 1000,
        efforts,
        effort: Some(text(b"high")),
    }
}
#[must_use]
#[expect(clippy::too_many_lines, reason = "independent fixtures supply every field of the four whole settings kinds")]
pub fn settings_values() -> List<SettingValue> {
    let mut values = List::with_capacity(4);
    let mut alternatives = List::with_capacity(1);
    let mut other = model();
    other.name = text(b"other");
    other.effort = None;
    alternatives.push(other).expect("one alternative");
    values
        .push(SettingValue::Charter(Box::new(Charter {
            documents: documents(),
            number: 2,
            instructions: text("instructions λ".as_bytes()),
            waiting: Duration::from_secs(11),
            resume: true,
            turns: 12,
            time: Duration::from_secs(13),
            reserve_turns: 2,
            reserve_time: Duration::from_secs(3),
            model: model(),
            alternatives,
            inspect: true,
            modify: false,
            shell: true,
            agents: false,
            call_timeout: Duration::from_secs(14),
            hosts: SettingHosts::Both,
            spend: 99,
        })))
        .expect("charter");
    values
        .push(SettingValue::Template(Box::new(Template {
            number: 1,
            name: DocumentText::complete(text(b"chat")),
            executor: TemplateExecutor::Charter(2),
            contract: TaskContractDocument::Report { words: 91 },
            shallower: 1,
            budget: TemplateBudget::AsAllowed { cap: Some(99) },
            deadline: TemplateDeadline::Span(Duration::from_secs(15)),
            tracked: Some(16),
            flags: TaskFlags {
                ended_by: EndedBy::Requester,
                conversation: true,
                listed: false,
                retention: Retention::ForgottenOnceRead,
            },
            label: 17,
            grace: Some(Duration::from_secs(18)),
            llm: Some(LlmChoice {
                account: Some(21),
                main: ModelEffort { model: text(b"model"), effort: Some(text(b"high")) },
                subagents: List::with_capacity(0),
            }),
        })))
        .expect("template");
    let mut roles = List::with_capacity(1);
    roles
        .push(PolicyEditRole {
            number: 1,
            authority: settings_authority(),
            period_spend: 101,
            requests: 255,
            decides: 7,
        })
        .expect("role");
    let mut requirements = List::with_capacity(1);
    requirements.push(requirement()).expect("requirement");
    let mut projections = List::with_capacity(1);
    projections.push(OwnerGrant { connector: 8, kind: 9, pattern: pattern() }).expect("projection");
    let mut permissions = List::with_capacity(1);
    permissions.push(PolicyPermission { connector: 10, permission: 11, role: 1 }).expect("permission");
    let mut templates = List::with_capacity(1);
    templates
        .push(TemplateOverride {
            template: 1,
            executor: Some(TemplateExecutor::Chosen),
            budget: Some(TemplateBudget::Amount(44)),
            deadline: Some(TemplateDeadline::Given),
            priority: Some(5),
        })
        .expect("override");
    values
        .push(SettingValue::Policy(Box::new(SettingPolicy {
            escalation_role: Some(1),
            ceiling: settings_authority(),
            projections,
            period_spend: 101,
            roles,
            requirements: requirements.clone(),
            permissions,
            templates,
        })))
        .expect("policy");
    let mut implies = List::with_capacity(1);
    implies.push(OwnerKindImplication { connector: 2, kind: 4, implies: 3 }).expect("implication");
    values
        .push(SettingValue::Rules(Box::new(Rules {
            ceiling: settings_authority(),
            period_spend: 202,
            minimum_run_spend: 3,
            maximum_run_spend: 90,
            implies,
            requirements,
        })))
        .expect("rules");
    values
}
#[must_use]
pub fn settings_names() -> [Setting; 4] {
    [
        Setting::Charter(SettingNumber { number: 2 }),
        Setting::Template(SettingNumber { number: 1 }),
        Setting::Policy(SettingNumber { number: 3 }),
        Setting::Rules,
    ]
}
#[must_use]
pub fn settings_change() -> ChangeSettings {
    let mut items = List::with_capacity(4);
    for (setting, value) in settings_names().into_iter().zip(settings_values().into_boxed()) {
        items.push(SettingItem { setting, edit: SettingEdit::Set(SettingSet { value }) }).expect("four settings");
    }
    ChangeSettings { against: 5, items, running: Running::Moves }
}
#[must_use]
pub fn settings_head() -> RevisionHead {
    RevisionHead {
        number: 6,
        against: 5,
        at: Wall::from_nanos(19),
        by: RevisionDecider::Party(RevisionParty { number: 7 }),
        proposal: Some(ProposalKey { proposer: Proposer::Party(7), number: 8 }),
        moved: true,
    }
}
#[must_use]
pub fn settings_document() -> SettingsDocument {
    let mut settings = List::with_capacity(4);
    for setting in settings_names() {
        settings
            .push(SettingLine {
                setting,
                name: if setting == Setting::Rules { None } else { Some(DocumentText::complete(text(b"name"))) },
                follows: SettingFollows::Change,
                last: 6,
            })
            .expect("four settings");
    }
    let mut held = List::with_capacity(1);
    held.push(HeldRevision { head: settings_head(), holders: 2 }).expect("held");
    SettingsDocument { newest: 6, settings, held, pending: List::with_capacity(0) }
}
#[must_use]
pub fn settings_revision() -> RevisionDocument {
    let mut items = List::with_capacity(7);
    for item in settings_change().items.into_boxed() {
        let SettingEdit::Set(value) = item.edit else { unreachable!("set fixture") };
        items.push(RevisionItem { setting: item.setting, edit: RevisionEdit::Set(value) }).expect("settings");
    }
    for edit in [RevisionEdit::Removed, RevisionEdit::Reset, RevisionEdit::TooLarge(Size { bytes: 999 })] {
        items.push(RevisionItem { setting: Setting::Charter(SettingNumber { number: 8 }), edit }).expect("edit");
    }
    RevisionDocument { head: settings_head(), items }
}
#[must_use]
pub fn settings_page() -> RevisionsPage {
    let mut items = List::with_capacity(3);
    for edit in [RevisionItemEdit::Set, RevisionItemEdit::Removed, RevisionItemEdit::Reset] {
        items.push(RevisionItemHead { setting: Setting::Rules, edit }).expect("heads");
    }
    let mut rows = List::with_capacity(1);
    rows.push(RevisionRow { head: settings_head(), items, more: 4 }).expect("row");
    RevisionsPage { rows, next: Some(4), forgotten: true }
}

fn documents() -> List<CharterDocument> {
    let mut documents = List::with_capacity(2);
    for heading in [Some(Box::from(b"Rules".as_slice())), None] {
        let mut name = List::with_capacity(1);
        name.push(Box::from(b"resource".as_slice())).expect("one name");
        documents
            .push(CharterDocument { connector: 1, name, path: Box::from(b"rules.md".as_slice()), heading })
            .expect("two documents");
    }
    documents
}
