#![expect(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::match_like_matches_macro,
    clippy::wildcard_enum_match_arm,
    reason = "ordinary proofs inspect original read tokens and whole keyed requests"
)]
use super::spine::{LIMITS, delivered, drive, frame, start};
use crate::tests::fixtures::{AnsweredByEngine, Fixture, ReadPage, Reported};
use crate::{
    Event, Request,
    action::Act,
    address::{Address, Place, ProjectSection},
    intents::Progress,
    page::project::owner_form,
};
use alloc::vec::Vec;
use jig_client::contract::*;
use skein_lib::{List, Time, Token, bytes::copy_of};
fn list<T>(cap: u32, rows: impl IntoIterator<Item = T>) -> List<T> {
    let mut out = List::with_capacity(cap);
    for row in rows {
        assert!(out.push(row).is_ok());
    }
    out
}
fn pattern() -> OwnerPattern {
    OwnerPattern {
        segments: list(2, [copy_of(&[255, 0, b'/']), copy_of(b"")]),
        last: OwnerPatternEnd::Open(copy_of(&[254, 0])),
    }
}
pub(super) fn source(number: u32) -> OwnerProject {
    let authority = OwnerAuthorityDocument::complete(OwnerAuthority {
        models: None,
        tools: u64::MAX,
        grants: list(2, [OwnerGrant { connector: u16::MAX, kind: u16::MAX, pattern: pattern() }]),
        delegation: OwnerDelegation {
            kinds: list(2, [OwnerAuthorityExecutor::Role(u32::MAX)]),
            tasks: u32::MAX,
            depth: u32::MAX,
        },
        spend: u64::MAX,
        deadline: None,
        notes: u8::MAX,
        note_resources: List::with_capacity(2),
    });
    OwnerProject {
        number,
        name: DocumentText::complete(Text::new(copy_of(b"Tally"), 32).unwrap()),
        acts: list(2, [Offered { effect: None, act: ActKind::ChangePolicy, admission: Admission::Allowed }]),
        resources: List::with_capacity(2),
        policy: OwnerProjectPolicy {
            templates: List::with_capacity(0),
            ceiling: authority.clone(),
            period_spend: u64::MAX,
            escalation_role: Some(u32::MAX),
            projections: list(2, [OwnerGrant { connector: 19, kind: 23, pattern: pattern() }]),
            roles: list(
                2,
                [OwnerProjectPolicyRole {
                    number: u32::MAX,
                    authority,
                    period_spend: 17,
                    requests: u16::MAX,
                    decides: u8::MAX,
                }],
            ),
            requirements: list(
                2,
                [PolicyRequirement {
                    connector: u16::MAX,
                    kind: u16::MAX,
                    pattern: pattern(),
                    judge: PolicyJudge { connector: u16::MAX, requirement: u16::MAX, parameters: u32::MAX },
                    guard: PolicyGuard::Guarded,
                    must_be_guarded: true,
                }],
            ),
            permissions: None,
            permissions_cut: false,
            cut: false,
        },
        members: List::with_capacity(2),
        current_period: None,
        pools: List::with_capacity(2),
        configured_period: 0,
        configured_pools: List::with_capacity(2),
        configured_pools_cut: false,
        spend: List::with_capacity(2),
        recurring: List::with_capacity(2),
        cut: false,
    }
}
fn read(out: &[Request]) -> jig_client::Handle {
    out.iter()
        .find_map(|r| match r {
            Request::Client {
                asked: jig_client::Event::Read { handle: read, read: jig_client::Readable::Jig(Read::Project { .. }) },
            } => Some(*read),
            _ => None,
        })
        .expect("original project read")
}
fn enter(domain: &mut Fixture, number: u32) -> Box<[Request]> {
    drive(
        domain,
        Time::ZERO,
        Event::Went { place: Place::Jig(Address::Project { project: number, section: ProjectSection::Settings }) },
    )
}
fn live(number: u32) -> (Fixture, jig_client::Handle) {
    let mut domain = Fixture::new(&LIMITS);
    let f = start(&mut domain, List::with_capacity(0));
    let mut value = frame(7);
    value.projects = list(2, [super::pages::project(number, false)]);
    delivered(&mut domain, Time::ZERO, f, value);
    let entered = enter(&mut domain, number);
    assert!(
        !entered.iter().any(|r| match r {
            Request::Client { asked: jig_client::Event::Watch { .. } } => true,
            _ => false,
        }),
        "settings does not acquire a Project watch"
    );
    let token = read(&entered);
    (domain, token)
}
fn install(domain: &mut Fixture, token: jig_client::Handle, value: OwnerProject) {
    drive(
        domain,
        Time::ZERO,
        ReadPage { read: token, result: ReadResult::Value(ReadValue::OwnerProject(Box::new(value))) },
    );
}
#[test]
fn owner_project_original_read_checks_full_identity_before_terminal_and_ignores_duplicate_late_and_reset_callbacks() {
    for number in [0, u32::MAX] {
        let (mut domain, token) = live(number);
        let value = source(number);
        let mut wrong = value.clone();
        wrong.number = if number == 0 { u32::MAX } else { 0 };
        install(&mut domain, token, wrong);
        assert!(domain.project_page().owner.is_none());
        assert!(domain.handles.reads.get(&token).is_none());
        let token = domain.read_again(token);
        install(&mut domain, token, value.clone());
        assert_eq!(domain.project_page().owner.as_deref(), Some(&value));
        assert!(domain.project_page().document.is_none());
        let mut duplicate = value.clone();
        duplicate.policy.period_spend = 0;
        install_late(&mut domain, token, duplicate.clone());
        assert_eq!(domain.project_page().owner.as_deref(), Some(&value));
        drive(&mut domain, Time::ZERO, Event::Went { place: Place::Elsewhere });
        let next = read(&enter(&mut domain, number));
        install_late(&mut domain, token, duplicate);
        assert!(domain.project_page().owner.is_none());
        install(&mut domain, next, value.clone());
        assert_eq!(domain.project_page().owner.as_deref(), Some(&value));
        drive(&mut domain, Time::ZERO, jig_client::Up::SignedOut);
        assert!(domain.project_page().owner.is_none());
        install_late(&mut domain, next, value);
        assert!(domain.project_page().owner.is_none());
    }
}
#[test]
fn owner_project_whole_role_review_keeps_raw_flags_authority_and_other_edited_channels_in_one_keyed_request() {
    let (mut domain, token) = live(u32::MAX);
    let value = source(u32::MAX);
    install(&mut domain, token, value.clone());
    let role = value.policy.roles.get(0).unwrap();
    let before = Ask::ChangePolicy {
        project: value.number,
        edit: PolicyEdit::Role(PolicyEditRole {
            number: role.number,
            authority: role.authority.whole().unwrap(),
            period_spend: role.period_spend,
            requests: role.requests,
            decides: role.decides,
        }),
    };
    let mut chosen = before.clone();
    if let Ask::ChangePolicy { edit: PolicyEdit::Role(role), .. } = &mut chosen {
        role.period_spend = 0;
        role.requests = 0x8041;
    }
    assert!(drive(&mut domain, Time::ZERO, Event::Act { action: owner_form::action(chosen.clone()) }).is_empty());
    assert_eq!(domain.project_page().settings.before.as_deref(), Some(&before));
    assert_eq!(owner_form::draft(&domain.project_page().settings, &before), chosen);
    let out = drive(&mut domain, Time::ZERO, Event::Act { action: Act::Confirm });
    let Request::Client {
        asked: jig_client::Event::Send { name: None, key: request, request: jig_client::Ask::Jig(ask) },
    } = &out[0]
    else {
        panic!("one keyed form request")
    };
    assert_eq!(domain.intents().get(skein_lib::Id::from_token(domain.intent(*request))).unwrap().scope, 7);
    assert_eq!(**ask, chosen);
    let request = *request;
    assert!(drive(&mut domain, Time::ZERO, Event::Act { action: Act::Confirm }).is_empty());
    drive(&mut domain, Time::ZERO, AnsweredByEngine { request, answer: Answer::Done(Outcome::Changed) });
    assert!(domain.project_page().settings.pending.is_none());
    assert_eq!(domain.project_page().owner.as_deref(), Some(&value));
}
#[test]
fn owner_project_policy_scalar_and_complete_lists_are_offered_without_inferred_permissions_or_cut_replacements() {
    let mut value = source(0);
    for edit in [
        PolicyEdit::Ceiling { ceiling: value.policy.ceiling.whole().unwrap() },
        PolicyEdit::ProjectSpend { period_spend: 0 },
        PolicyEdit::Requirements { requirements: value.policy.requirements.clone() },
        PolicyEdit::Projections { grants: value.policy.projections.clone() },
    ] {
        assert!(owner_form::supported(&value, &Ask::ChangePolicy { project: 0, edit }));
    }
    let absent = Ask::ChangePolicy { project: 0, edit: PolicyEdit::Permissions { mappings: List::with_capacity(2) } };
    assert!(!owner_form::supported(&value, &absent));
    assert_eq!(owner_form::baseline(&value, &absent), None);
    value.policy.cut = true;
    assert!(!owner_form::supported(
        &value,
        &Ask::ChangePolicy { project: 0, edit: PolicyEdit::Projections { grants: value.policy.projections.clone() } }
    ));
    assert!(owner_form::supported(
        &value,
        &Ask::ChangePolicy { project: 0, edit: PolicyEdit::ProjectSpend { period_spend: 0 } }
    ));
    let (mut domain, token) = live(0);
    install(&mut domain, token, value);
    assert!(drive(&mut domain, Time::ZERO, Event::Act { action: owner_form::action(absent) }).is_empty());
    assert!(drive(&mut domain, Time::ZERO, Event::Act { action: Act::Confirm }).is_empty());
}
#[test]
fn owner_project_configured_pool_review_keeps_zero_target_distinct_and_refuses_changed_period_or_cut_absence() {
    let mut value = source(0);
    value.acts = list(2, [Offered { effect: None, act: ActKind::SetPool, admission: Admission::Allowed }]);
    let numbers = OwnerNumbers { budget: 11, spent: 2, spent_below: 3, reserved: 4 };
    let observed = PoolLedger {
        source: Funder::Pool { project: 0, person: u64::MAX, period: 9 },
        parent: Some(Funder::Period { project: 0, period: 9 }),
        numbers,
        left: 2,
        closed: false,
        began: Some(skein_lib::Wall::EPOCH),
        next: None,
    };
    value.pools.push(observed.clone()).unwrap();
    value
        .configured_pools
        .push(PoolLedger {
            source: Funder::Pool { project: 0, person: u64::MAX, period: 0 },
            parent: Some(Funder::Period { project: 0, period: 0 }),
            numbers: OwnerNumbers { budget: 71, ..numbers },
            closed: true,
            ..observed
        })
        .unwrap();
    let chosen = Ask::SetPool { project: 0, party: u64::MAX, budget: u64::MAX };
    let before = Ask::SetPool { project: 0, party: u64::MAX, budget: 71 };
    assert_eq!(owner_form::baseline(&value, &chosen), Some(before.clone()));
    let (mut domain, token) = live(0);
    install(&mut domain, token, value.clone());
    drive(&mut domain, Time::ZERO, Event::Act { action: owner_form::action(chosen.clone()) });
    assert_eq!(domain.project_page().settings.pool_period, Some(0));
    assert_eq!(domain.project_page().settings.before.as_deref(), Some(&before));
    let sent = drive(&mut domain, Time::ZERO, Event::Act { action: Act::Confirm });
    assert!(match &sent[..] {
        [Request::Client { asked: jig_client::Event::Send { request: jig_client::Ask::Jig(ask), .. } }]
            if ask.as_ref() == &chosen =>
            true,
        _ => false,
    });
    let absent = Ask::SetPool { project: 0, party: 0, budget: 113 };
    assert!(owner_form::supported(&value, &absent));
    assert_eq!(owner_form::baseline(&value, &absent), None);
    let (mut domain, token) = live(0);
    install(&mut domain, token, value.clone());
    drive(&mut domain, Time::ZERO, Event::Act { action: owner_form::action(absent.clone()) });
    domain.project_page.owner.as_mut().unwrap().configured_period = u64::MAX;
    assert!(drive(&mut domain, Time::ZERO, Event::Act { action: Act::Confirm }).is_empty());
    value.configured_pools_cut = true;
    assert!(!owner_form::supported(&value, &absent));
    assert_eq!(value.pools.get(0).unwrap().numbers.budget, 11);
    assert_eq!(value.configured_pools.get(0).unwrap().numbers.budget, 71);
}
#[test]
fn owner_project_permission_review_preserves_full_width_order_and_distinguishes_absent_empty_and_cut() {
    let mut value = source(u32::MAX);
    let mappings = list(
        2,
        [
            PolicyPermission { connector: u16::MAX, permission: u16::MAX, role: u32::MAX },
            PolicyPermission { connector: 0, permission: 0, role: 0 },
        ],
    );
    let chosen =
        Ask::ChangePolicy { project: value.number, edit: PolicyEdit::Permissions { mappings: List::with_capacity(2) } };
    assert!(!owner_form::supported(&value, &chosen));
    value.policy.permissions = Some(mappings.clone());
    let before = Ask::ChangePolicy { project: value.number, edit: PolicyEdit::Permissions { mappings } };
    assert_eq!(owner_form::baseline(&value, &chosen), Some(before.clone()));
    let (mut domain, token) = live(value.number);
    install(&mut domain, token, value.clone());
    drive(&mut domain, Time::ZERO, Event::Act { action: owner_form::action(chosen.clone()) });
    assert_eq!(domain.project_page().settings.before.as_deref(), Some(&before));
    let sent = drive(&mut domain, Time::ZERO, Event::Act { action: Act::Confirm });
    assert!(match &sent[..] {
        [Request::Client { asked: jig_client::Event::Send { request: jig_client::Ask::Jig(ask), .. } }]
            if ask.as_ref() == &chosen =>
            true,
        _ => false,
    });
    value.policy.permissions = Some(List::with_capacity(2));
    assert_eq!(owner_form::baseline(&value, &chosen), Some(chosen.clone()));
    value.policy.permissions_cut = true;
    assert!(!owner_form::supported(&value, &chosen));
    value.policy.permissions_cut = false;
    value.policy.cut = true;
    assert!(!owner_form::supported(&value, &chosen));
    value.policy.cut = false;
    value.cut = true;
    assert!(!owner_form::supported(&value, &chosen));
}
#[test]
fn owner_project_service_requires_explicit_role_before_name_without_emitting_an_implicit_request() {
    let mut value = source(0);
    value.acts = list(2, [Offered { effect: None, act: ActKind::MakeService, admission: Admission::Allowed }]);
    let (mut domain, token) = live(0);
    install(&mut domain, token, value.clone());
    assert!(domain.project_page().settings.service_role.is_none());
    assert!(
        drive(
            &mut domain,
            Time::ZERO,
            Event::Act { action: Act::ProjectServiceRole { role: HeldRole::Policy { role: 17 } } }
        )
        .is_empty()
    );
    assert!(domain.project_page().settings.service_role.is_none());
    let role = HeldRole::Policy { role: u32::MAX };
    assert!(drive(&mut domain, Time::ZERO, Event::Act { action: Act::ProjectServiceRole { role } }).is_empty());
    assert_eq!(domain.project_page().settings.service_role, Some(role));
    assert!(domain.project_page().settings.review.is_none());
    let chosen = Ask::MakeService { project: 0, name: Text::new(copy_of(b""), LIMITS.text).unwrap(), role };
    assert!(drive(&mut domain, Time::ZERO, Event::Act { action: owner_form::action(chosen.clone()) }).is_empty());
    let sent = drive(&mut domain, Time::ZERO, Event::Act { action: Act::Confirm });
    assert!(match &sent[..] {
        [Request::Client { asked: jig_client::Event::Send { request: jig_client::Ask::Jig(ask), .. } }]
            if ask.as_ref() == &chosen =>
            true,
        _ => false,
    });
}

#[test]
fn owner_project_settings_unknown_retires_only_matching_pending_and_late_answer_never_reopens_or_resends() {
    let (mut domain, token) = live(0);
    let value = source(0);
    install(&mut domain, token, value.clone());
    let ask = Ask::ChangePolicy { project: 0, edit: PolicyEdit::ProjectSpend { period_spend: 19 } };
    assert!(drive(&mut domain, Time::ZERO, Event::Act { action: owner_form::action(ask) }).is_empty());
    let out = drive(&mut domain, Time::ZERO, Event::Act { action: Act::Confirm });
    let request = out
        .iter()
        .find_map(|row| match row {
            Request::Client { asked: jig_client::Event::Send { key: request, .. } } => Some(*request),
            _ => None,
        })
        .unwrap();
    owner_form::unknown(&mut domain, Token::new(u64::MAX));
    assert_eq!(domain.project_page().settings.pending, Some(domain.intent(request)));
    assert!(drive(&mut domain, Time::ZERO, Reported { request, progress: Progress::Unknown }).is_empty());
    assert!(domain.project_page().settings.pending.is_none());
    let notices: Vec<_> =
        domain.notices().ids().into_iter().map(|(id, ())| domain.notices().get(*id).unwrap().kind.clone()).collect();
    assert!(
        notices.contains(&crate::notices::kept::Kind::PageUnknown {
            purpose: crate::page::IntentPurpose::ProjectSettings
        })
    );
    assert!(drive(&mut domain, Time::ZERO, Reported { request, progress: Progress::Unknown }).is_empty());
    assert!(
        drive(&mut domain, Time::ZERO, AnsweredByEngine { request, answer: Answer::Done(Outcome::Changed) }).is_empty()
    );
    assert!(drive(&mut domain, Time::ZERO, Event::Act { action: Act::Confirm }).is_empty());
    let after: Vec<_> =
        domain.notices().ids().into_iter().map(|(id, ())| domain.notices().get(*id).unwrap().kind.clone()).collect();
    assert_eq!(after, notices);
    assert_eq!(domain.project_page().owner.as_deref(), Some(&value));
}

fn install_late(domain: &mut Fixture, read: jig_client::Handle, value: OwnerProject) {
    drive(
        domain,
        Time::ZERO,
        ReadPage { read, result: ReadResult::Value(ReadValue::OwnerProject(Box::new(value))) }.late(),
    );
}
