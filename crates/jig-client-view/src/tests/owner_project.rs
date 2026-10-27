#![expect(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::match_like_matches_macro,
    clippy::wildcard_enum_match_arm,
    reason = "ordinary public proofs inspect complete accessible trees, raw slots and decoded actions"
)]
use crate::tests::fixtures::{Fixture, ReadPage, Served, Watched};
use crate::words::Words;
use crate::{JigBindings, owner_project as view};
use jig_client::thread::window::End;
use skein_ui::phrases::{PhraseLimits, Phrasebook, Segment, SpanUnit, WallFormat};
use skein_ui::{Builder, Role, Tree};

use jig_client::contract::*;
use skein_lib::{
    Duration, List, Wall,
    bytes::{copy_of, find},
};
fn list<T>(cap: u32, rows: impl IntoIterator<Item = T>) -> List<T> {
    let mut out = List::with_capacity(cap);
    for row in rows {
        assert!(out.push(row).is_ok());
    }
    out
}
fn limits() -> crate::Limits {
    crate::Limits { rows: 4, text: 32, ..super::cards::CLIENT }
}
fn doc(bytes: &[u8], cut: bool) -> DocumentText {
    DocumentText { bytes: Text::new(copy_of(bytes), 32).unwrap(), cut }
}
fn pattern() -> OwnerPattern {
    OwnerPattern {
        segments: list(4, [copy_of(&[255, 0, b'/']), copy_of(b"a/b"), copy_of(b"")]),
        last: OwnerPatternEnd::Open(copy_of(&[254, 0, b'/'])),
    }
}
fn part() -> OwnerPart {
    OwnerPart { connector: u16::MAX, kind: OwnerPartKind::Outbox, key: OwnerPartKey::Path(pattern().segments) }
}
fn authority(cut: bool) -> OwnerAuthorityDocument {
    OwnerAuthorityDocument {
        authority: OwnerAuthority {
            models: None,
            tools: u64::MAX,
            grants: list(4, [OwnerGrant { connector: u16::MAX, kind: u16::MAX, pattern: pattern() }]),
            delegation: OwnerDelegation {
                kinds: list(
                    4,
                    [
                        OwnerAuthorityExecutor::Charter(u32::MAX),
                        OwnerAuthorityExecutor::Procedure(u32::MAX),
                        OwnerAuthorityExecutor::Role(u32::MAX),
                    ],
                ),
                tasks: u32::MAX,
                depth: u32::MAX,
            },
            spend: u64::MAX,
            deadline: Some(Wall::EPOCH),
            notes: u8::MAX,
            note_resources: list(4, [OwnerResourceScope { connector: u16::MAX, pattern: pattern() }]),
        },
        cut,
    }
}
fn numbers() -> OwnerNumbers {
    OwnerNumbers { budget: 101, spent: 103, spent_below: 107, reserved: 109 }
}
fn ledger(source: Funder) -> PoolLedger {
    PoolLedger {
        source,
        parent: Some(Funder::Task(u64::MAX)),
        numbers: numbers(),
        left: 113,
        closed: true,
        began: Some(Wall::EPOCH),
        next: None,
    }
}
fn chip(number: u64) -> Chip {
    Chip {
        task: number,
        title: doc(b"source chip", true),
        executor: Executor::Role { project: u32::MAX, role: u32::MAX },
        phase: Phase::Preparing,
        details: Box::new(ChipPhase::Owner {
            phase: OwnerChipPhase::Active(RunStage::Preparing),
            details: Box::new(super::owner_chip::details()),
        }),
    }
}
fn template(number: u64) -> ProposedTask {
    ProposedTask {
        deadline: NewDeadline::Named,
        number,
        spec: SpecificationDocument {
            specification: DocumentSpecification {
                words: doc(b"full template source", true),
                parameters: list(4, [ParameterDocument::Bytes { name: u32::MAX, value: doc(UTF8_TEXTS[3], true) }]),
                inputs: list(4, [u64::MAX, 0]),
            },
            cut: true,
        },
        executor: OwnerExecutor::Person(PersonAddress::Role(u32::MAX)),
        contract: TaskContractDocument::Approval { state: copy_of(&[255, 0, 254]), part: part() },
        authority: authority(true),
        budget: u64::MAX,
        funder: Funder::Recurring { project: u32::MAX, task: u64::MAX, period: 0 },
        dependencies: list(4, [u64::MAX, 0]),
        resources: list(4, [part()]),
        wake: OwnerWakePolicy {
            markers: 0,
            words: WakeRule::Immediate,
            notices: WakeRule::Never,
            news: WakeRule::Never,
            results: ResultsWake::Each,
            questions: true,
            answers: true,
            timers: true,
        },
        priority: Some(u32::MAX),
        cut: true,
    }
}
#[expect(clippy::too_many_lines, reason = "complete whole source fixture or ordered public journey proof")]
fn source() -> OwnerProject {
    let funders = [
        Funder::Task(u64::MAX),
        Funder::Pool { project: u32::MAX, person: u64::MAX, period: 0 },
        Funder::Recurring { project: 0, task: u64::MAX, period: u64::MAX },
        Funder::Period { project: u32::MAX, period: u64::MAX },
    ];
    OwnerProject {
        number: u32::MAX,
        name: doc(b"Tally", false),
        acts: List::with_capacity(4),
        resources: list(
            4,
            [
                OwnerProjectResourceRole::Owned,
                OwnerProjectResourceRole::Participant,
                OwnerProjectResourceRole::Unavailable,
                OwnerProjectResourceRole::Context,
            ]
            .map(|role| OwnerProjectResource { part: part(), role }),
        ),
        policy: OwnerProjectPolicy {
            templates: List::with_capacity(0),
            ceiling: authority(true),
            period_spend: u64::MAX,
            escalation_role: None,
            projections: list(
                4,
                [
                    OwnerGrant { connector: u16::MAX, kind: u16::MAX, pattern: pattern() },
                    OwnerGrant { connector: 0, kind: 0, pattern: pattern() },
                ],
            ),
            roles: list(
                4,
                [OwnerProjectPolicyRole {
                    number: u32::MAX,
                    authority: authority(true),
                    period_spend: u64::MAX,
                    requests: u16::MAX,
                    decides: u8::MAX,
                }],
            ),
            requirements: list(
                4,
                [
                    PolicyRequirement {
                        connector: u16::MAX,
                        kind: u16::MAX,
                        pattern: pattern(),
                        judge: PolicyJudge { connector: u16::MAX, requirement: u16::MAX, parameters: u32::MAX },
                        guard: PolicyGuard::Observed { freshness: Duration::from_nanos(u64::MAX) },
                        must_be_guarded: true,
                    },
                    PolicyRequirement {
                        connector: 0,
                        kind: 0,
                        pattern: pattern(),
                        judge: PolicyJudge { connector: 0, requirement: 0, parameters: 0 },
                        guard: PolicyGuard::Guarded,
                        must_be_guarded: false,
                    },
                ],
            ),
            permissions: None,
            permissions_cut: false,
            cut: true,
        },
        members: list(
            4,
            [
                OwnerProjectMember {
                    party: u64::MAX,
                    kind: PartyKind::Service,
                    role: HeldRole::Policy { role: u32::MAX },
                    login: doc(UTF8_TEXTS[3], true),
                    name: doc(b"", false),
                },
                OwnerProjectMember {
                    party: 0,
                    kind: PartyKind::Person,
                    role: HeldRole::Owner,
                    login: doc(b"", false),
                    name: doc(UTF8_TEXTS[4], true),
                },
            ],
        ),
        current_period: None,
        pools: list(4, funders.map(ledger)),
        configured_period: 0,
        configured_pools: List::with_capacity(4),
        configured_pools_cut: false,
        spend: list(
            4,
            funders.into_iter().enumerate().map(|(index, funder)| OwnerProjectSpend {
                chip: chip([9, 0, u64::MAX, 9][index]),
                funder,
                numbers: numbers(),
            }),
        ),
        recurring: list(
            4,
            [Recurring {
                chip: chip(u64::MAX),
                key: u32::MAX,
                batch: list(4, [template(u64::MAX), template(0)]),
                budget: u64::MAX,
                overlap: RecurringOverlap::Wait,
                current_period: 0,
                began: Some(Wall::EPOCH),
                next: None,
                last_period: u64::MAX,
                pending_period: Some(0),
                last_batch: list(4, [u64::MAX, 0, u64::MAX]),
                cut: true,
            }],
        ),
        cut: true,
    }
}
fn make_words(selected: bool) -> Words {
    let mut first = crate::phrases::english::language();
    first.wall = WallFormat::Nanos;
    first.span_unit = SpanUnit::Nanos;
    let mut second = first.clone();
    for p in &mut second.phrases {
        for f in &mut p.forms {
            for s in &mut f.segments {
                if let Segment::Text(bytes) = s {
                    *bytes = copy_of(b"second ");
                }
            }
        }
    }
    Words::new(
        Phrasebook::load(
            crate::phrases::jig::shapes(32),
            Box::new([first, second]),
            &PhraseLimits {
                languages: 2,
                phrases: crate::owner_words::phrase_count(),
                holes: 4,
                segments: 10000,
                text_bytes: 200_000,
                hole_bytes: 32,
            },
        )
        .unwrap(),
        u16::from(selected),
        0,
    )
}
fn words(selected: bool) -> &'static Words {
    static NORMAL: std::sync::OnceLock<Words> = std::sync::OnceLock::new();
    static SELECTED: std::sync::OnceLock<Words> = std::sync::OnceLock::new();
    if selected { SELECTED.get_or_init(|| make_words(true)) } else { NORMAL.get_or_init(|| make_words(false)) }
}
fn capacity(f: view::ProjectFragment) -> skein_ui::Limits {
    skein_ui::Limits {
        nodes: f.nodes,
        depth: f.depth,
        text_per_node: f.text_per_node,
        text_bytes: f.text_bytes,
        drawing_bytes: 0,
        styles_per_node: 2,
        patches: 0,
        bindings: f.bindings,
        slots: f.slots,
        markdown_depth: 4,
        scene_marks: 0,
        scene_bytes: 0,
    }
}
fn build(value: &OwnerProject, w: &Words) -> (Tree, JigBindings) {
    let f = view::required(value, &limits(), w).expect("whole finite admission");
    let mut b = Builder::new(&capacity(f));
    let mut bindings = JigBindings::new(f.bindings, 32);
    let d = Fixture::new(&limits());
    view::render(&mut b, value, &d.view(), w, &mut bindings);
    let tree = b.finish();
    super::cards_access::check(&tree);
    assert!(tree.len() <= f.nodes);
    let bytes: u64 = tree
        .nodes()
        .iter()
        .map(|n| {
            assert!(crate::tests::text_bytes(n) <= f.text_per_node);
            u64::from(crate::tests::text_bytes(n))
        })
        .sum();
    assert!(bytes <= u64::from(f.text_bytes));
    (tree, bindings)
}
fn has(tree: &Tree, bytes: &[u8]) -> bool {
    tree.nodes().iter().any(|n| n.text.iter().any(|r| find(&r.text, bytes).is_some()))
}
#[test]
fn owner_project_public_whole_view_keeps_four_resource_roles_opaque_raw_slots_members_and_independent_losses() {
    let value = source();
    let before = value.clone();
    let (tree, bindings) = build(&value, words(false));
    assert!(
        tree.nodes().iter().any(|node| node.role == Role::Region && node.name.as_deref() == Some(b"Project: Tally"))
    );
    for label in [
        b"Owned".as_slice(),
        b"Participant",
        b"Unavailable",
        b"Context",
        b"Service",
        b"Person",
        b"Project document is incomplete",
        b"Policy is incomplete",
        b"Permission mappings not supplied",
        b"Escalation role not supplied",
    ] {
        assert!(has(&tree, label), "missing {label:?}");
    }
    let raw: Vec<_> = tree.nodes().iter().filter_map(|n| n.slot.and_then(|s| bindings.owner_part(s.token))).collect();
    assert!(raw.len() >= 4);
    for actual in raw {
        assert_eq!(actual, &part());
    }
    assert!(
        tree.nodes()
            .iter()
            .any(|n| n.role == Role::Region && n.name.as_deref() == Some(b"Member: 18446744073709551615"))
    );
    assert_eq!(value, before);
}
#[test]
fn owner_project_optional_ledger_distinguishes_none_supplied_empty_epoch_and_all_four_funding_counters() {
    let mut value = source();
    let (absent, _) = build(&value, words(false));
    assert!(has(&absent, b"Current period not supplied"));
    value.current_period = Some(ledger(Funder::Period { project: 0, period: 0 }));
    let (tree, _) = build(&value, words(false));
    assert!(!has(&tree, b"Current period not supplied"));
    for n in [b"101".as_slice(), b"103", b"107", b"109", b"113", b"18446744073709551615"] {
        assert!(has(&tree, n));
    }
    let supplied = value.current_period.as_ref().unwrap();
    assert_eq!(supplied.began, Some(Wall::EPOCH));
    let mut empty = supplied.clone();
    empty.numbers = OwnerNumbers { budget: 0, spent: 0, spent_below: 0, reserved: 0 };
    empty.left = 0;
    value.current_period = Some(empty);
    let (empty_tree, _) = build(&value, words(false));
    assert!(!has(&empty_tree, b"Current period not supplied"));
    assert_eq!(value.current_period.as_ref().unwrap().numbers.budget, 0);
    assert_eq!(value.current_period.as_ref().unwrap().began, Some(Wall::EPOCH));
    assert_eq!(value.current_period.as_ref().unwrap().next, None);
    assert!(value.current_period.as_ref().unwrap().closed);
    for source in [
        Funder::Task(u64::MAX),
        Funder::Pool { project: u32::MAX, person: u64::MAX, period: 0 },
        Funder::Recurring { project: 0, task: u64::MAX, period: u64::MAX },
        Funder::Period { project: u32::MAX, period: u64::MAX },
    ] {
        assert!(value.pools.iter().any(|p| p.source == source));
    }
}
#[test]
fn owner_project_raw_role_permissions_requirement_guards_and_spend_recurring_source_order_are_public_facts() {
    let value = source();
    let (tree, _) = build(&value, words(false));
    for n in [
        b"65535".as_slice(),
        b"255",
        b"4294967295",
        b"18446744073709551615",
        b"full template source",
        b"Some recurring template facts were omitted",
    ] {
        assert!(has(&tree, n), "missing {n:?}");
    }
    let (_, bindings) = build(&value, words(false));
    let displayed: Vec<_> = tree
        .nodes()
        .iter()
        .enumerate()
        .filter(|(index, node)| {
            node.role == Role::Region
                && node.name.as_deref() == Some(b"Spend".as_slice())
                && index.checked_sub(1).is_some_and(|parent| tree.nodes()[parent].role == Role::Item)
        })
        .map(|(index, node)| {
            tree.nodes()[index..index + usize::try_from(node.size).unwrap()]
                .iter()
                .find_map(|child| child.binding.and_then(|binding| bindings.address(binding.token)))
                .expect("source expense has its named task link")
        })
        .map(|address| match address {
            crate::address::Address::Task { task, .. } => task,
            _ => panic!("expense task link"),
        })
        .collect();
    assert_eq!(displayed, [9, 0, u64::MAX, 9]);
    assert_eq!(value.spend.iter().map(|s| s.chip.task).collect::<Vec<_>>(), [9, 0, u64::MAX, 9]);
    assert_eq!(value.recurring.get(0).unwrap().batch.iter().map(|t| t.number).collect::<Vec<_>>(), [u64::MAX, 0]);
    let region_name = words(false)
        .say(crate::phrases::jig::JigPhrase::RecurringKey, &[skein_ui::phrases::Hole::Number(u64::from(u32::MAX))]);
    let (index, recurring) = tree
        .nodes()
        .iter()
        .enumerate()
        .find(|(_, node)| node.role == Role::Region && node.name.as_deref() == Some(region_name.as_ref()))
        .unwrap();
    let last_links: Vec<_> = tree.nodes()[index..index + usize::try_from(recurring.size).unwrap()]
        .iter()
        .filter(|node| {
            node.role == Role::Link
                && node.name.as_deref().is_some_and(|name| {
                    [0, u64::MAX].into_iter().any(|task| {
                        name == words(false)
                            .say(crate::phrases::jig::JigPhrase::TaskPage, &[skein_ui::phrases::Hole::Number(task)])
                            .as_ref()
                    })
                })
        })
        .filter_map(|node| node.binding.and_then(|binding| bindings.address(binding.token)))
        .map(|address| match address {
            crate::address::Address::Task { task, .. } => task,
            _ => panic!("recurring task link"),
        })
        .collect();
    assert_eq!(last_links, [u64::MAX, 0, u64::MAX]);
    assert_eq!(value.recurring.get(0).unwrap().last_batch.as_slice(), [u64::MAX, 0, u64::MAX]);
    assert_eq!(value.resources.capacity(), 4);
    assert_eq!(value.recurring.get(0).unwrap().batch.capacity(), 4);
    assert_eq!(
        value.policy.requirements.get(0).unwrap().guard,
        PolicyGuard::Observed { freshness: Duration::from_nanos(u64::MAX) }
    );
}
#[test]
fn owner_project_public_required_envelope_checks_last_nested_capacity_and_both_loaded_languages_without_clipping() {
    let value = source();
    let before = value.clone();
    for selected in [false, true] {
        let w = words(selected);
        let f = view::required(&value, &limits(), w).unwrap();
        let (tree, _) = build(&value, w);
        assert!(tree.len() <= f.nodes);
        assert!(crate::jig_bindings_worst_case(f.bindings, &limits()).is_some());
        assert_eq!(has(&tree, b"second "), selected);
    }
    let w = words(false);
    let mut small = limits();
    small.rows = 3;
    small.segments = 3;
    assert!(view::required(&value, &small, w).is_none());
    let mut changed = value.clone();
    changed.recurring.get_mut(0).unwrap().batch.get_mut(1).unwrap().resources = List::with_capacity(5);
    assert!(view::required(&changed, &limits(), w).is_none());
    let mut changed = value.clone();
    changed.members.get_mut(1).unwrap().name = doc(UTF8_TEXTS[5], true);
    assert!(view::required(&changed, &limits(), w).is_some());
    let mut small = limits();
    small.text = 31;
    assert!(view::required(&changed, &small, w).is_none());
    assert_eq!(value, before);
}
fn native_drive(domain: &mut Fixture, event: impl crate::tests::fixtures::Input) -> Vec<crate::Request> {
    domain.drive(skein_lib::Time::ZERO, event).into_vec()
}
#[test]
fn owner_project_real_page_decodes_whole_role_edits_preserves_prior_channels_and_reviews_before_after_once() {
    check_owner_project_real_page_decodes_whole_role_edits_preserves_prior_channels_and_reviews_before_after_once();
}
#[expect(clippy::too_many_lines, reason = "complete whole source fixture or ordered public journey proof")]
fn check_owner_project_real_page_decodes_whole_role_edits_preserves_prior_channels_and_reviews_before_after_once() {
    use crate::{
        Event, Request,
        action::Act,
        address::{Address, ProjectSection},
    };
    let mut d = Fixture::new(&limits());
    let opened = native_drive(
        &mut d,
        Event::Start {
            seed: 8,
            place: crate::address::Place::Jig(Address::Project { project: 4, section: ProjectSection::Settings }),
            records: List::with_capacity(0),
        },
    );
    native_drive(&mut d, Served { served: true });
    let f = opened
        .iter()
        .find_map(|r| match r {
            Request::Client {
                asked: jig_client::Event::Watch { handle: watch, subject: jig_client::Watchable::Jig(Subject::Me) },
            } => Some(*watch),
            _ => None,
        })
        .unwrap();
    let ready = native_drive(
        &mut d,
        Watched { watch: f, event: WatchEvent::Delivered(Delivery::Me(Box::new(super::cards::document()))) },
    );
    let read =
        ready
            .iter()
            .find_map(|r| match r {
                Request::Client {
                    asked:
                        jig_client::Event::Read {
                            handle: read,
                            read: jig_client::Readable::Jig(Read::Project { project: 4 }),
                        },
                } => Some(*read),
                _ => None,
            })
            .unwrap();
    let mut value = source();
    value.number = 4;
    value.cut = false;
    value.policy.cut = false;
    value.policy.ceiling.cut = false;
    value.policy.roles.get_mut(0).unwrap().authority.cut = false;
    value.acts.push(Offered { effect: None, act: ActKind::ChangePolicy, admission: Admission::Allowed }).unwrap();
    native_drive(
        &mut d,
        ReadPage { read, result: ReadResult::Value(ReadValue::OwnerProject(Box::new(value.clone()))) },
    );
    let w = words(false);
    let f = view::worst_case(&limits(), w).unwrap();
    let mut bounds = capacity(f);
    bounds.nodes = bounds.nodes.checked_add(32).unwrap();
    bounds.text_bytes = bounds.text_bytes.checked_add(bounds.text_per_node.checked_mul(32).unwrap()).unwrap();
    let page = |d: &Fixture| {
        let mut b = Builder::new(&bounds);
        let mut bindings = JigBindings::new(f.bindings, 32);
        crate::pages::main(&mut b, &d.view(), w, &mut bindings);
        let tree = b.finish();
        super::cards_access::check(&tree);
        (tree, bindings)
    };
    let (tree, mut bindings) = page(&d);
    let textbox = tree
        .nodes()
        .iter()
        .find(|n| n.role == Role::TextField && n.name.as_deref() == Some(b"Request permission bits 65535".as_slice()))
        .or_else(|| {
            tree.nodes().iter().find(|n| {
                n.role == Role::TextField && n.name.as_ref().is_some_and(|name| find(name, b"65535").is_some())
            })
        })
        .expect("whole requests textbox")
        .binding
        .unwrap()
        .token;
    let act = crate::decode(&mut bindings, textbox, skein_ui::Act::Input { value: copy_of(b"32833") }).unwrap();
    assert!(native_drive(&mut d, Event::Act { action: act }).is_empty());
    let (tree, mut bindings) = page(&d);
    let field = tree
        .nodes()
        .iter()
        .find(|n| n.role == Role::TextField && n.name.as_ref().is_some_and(|name| find(name, b"255").is_some()))
        .expect("decides textbox")
        .binding
        .unwrap()
        .token;
    let act = crate::decode(&mut bindings, field, skein_ui::Act::Input { value: copy_of(b"129") }).unwrap();
    native_drive(&mut d, Event::Act { action: act });
    let (tree, mut bindings) = page(&d);
    assert!(has(&tree, b"Before"));
    assert!(has(&tree, b"After"));
    assert!(has(&tree, b"32833"));
    assert!(has(&tree, b"129"));
    let confirm = tree
        .nodes()
        .iter()
        .find(|n| n.role == Role::Button && n.name.as_deref() == Some(b"Confirm".as_slice()))
        .unwrap()
        .binding
        .unwrap()
        .token;
    let act = crate::decode(&mut bindings, confirm, skein_ui::Act::Press).unwrap();
    assert_eq!(act, Act::Confirm);
    let sent = native_drive(&mut d, Event::Act { action: act });
    let asks: Vec<_> = sent
        .iter()
        .filter_map(|r| match r {
            Request::Client { asked: jig_client::Event::Send { request: jig_client::Ask::Jig(ask), .. } } => {
                Some(ask.as_ref())
            }
            _ => None,
        })
        .collect();
    assert_eq!(asks.len(), 1);
    let Ask::ChangePolicy { project, edit: PolicyEdit::Role(role) } = asks[0] else { panic!("one whole role") };
    assert_eq!(*project, 4);
    assert_eq!(role.requests, 32833);
    assert_eq!(role.decides, 129);
    assert_eq!(role.authority, value.policy.roles.get(0).unwrap().authority.authority);
    assert_eq!(role.period_spend, u64::MAX);
    assert!(native_drive(&mut d, Event::Act { action: Act::Confirm }).is_empty());
}
#[test]
fn owner_project_public_baselines_preserve_optional_permissions_configured_targets_and_empty_recurring_titles() {
    let mut value = source();
    value.configured_period = 0;
    value.configured_pools.push(ledger(Funder::Pool { project: u32::MAX, person: 0, period: 0 })).unwrap();
    value.policy.permissions = Some(list(
        4,
        [
            PolicyPermission { connector: u16::MAX, permission: u16::MAX, role: u32::MAX },
            PolicyPermission { connector: 0, permission: 0, role: 0 },
        ],
    ));
    value.policy.permissions_cut = true;
    value.recurring.get_mut(0).unwrap().chip.title = doc(b"", true);
    let before = value.clone();
    let (tree, _) = build(&value, words(false));
    assert!(has(&tree, b"Configured pool period: 0"));
    assert!(!has(&tree, b"Permission mappings not supplied"));
    assert!(has(&tree, b"65535"));
    assert!(has(&tree, b"4294967295"));
    assert!(
        tree.nodes().iter().any(|node| node.role == Role::Region
            && node.name.as_ref().is_some_and(|name| find(name, b"4294967295").is_some()))
    );
    assert_eq!(value, before);
    value.policy.permissions = Some(List::with_capacity(4));
    let (tree, _) = build(&value, words(false));
    assert!(!has(&tree, b"Permission mappings not supplied"));
    value.policy.permissions = None;
    let (tree, _) = build(&value, words(false));
    assert!(has(&tree, b"Permission mappings not supplied"));
    value.configured_pools = List::with_capacity(5);
    assert!(view::required(&value, &limits(), words(false)).is_none());
    value.configured_pools = List::with_capacity(4);
    value.policy.permissions = Some(List::with_capacity(5));
    assert!(view::required(&value, &limits(), words(false)).is_none());
}
#[test]
#[expect(clippy::assertions_on_result_states, reason = "assert both independent one-under builder refusals")]
fn owner_project_public_whole_tree_accepts_exact_observed_nodes_and_text_and_refuses_one_under_without_clipping() {
    let value = source();
    let w = words(false);
    let (tree, _) = build(&value, w);
    let f = view::required(&value, &limits(), w).unwrap();
    let mut bound = capacity(f);
    bound.nodes = tree.len();
    bound.text_bytes = tree.nodes().iter().map(crate::tests::text_bytes).sum();
    let render = |bound: skein_ui::Limits| {
        let mut b = Builder::new(&bound);
        let mut bindings = JigBindings::new(f.bindings, 32);
        view::render(&mut b, &value, &Fixture::new(&limits()).view(), w, &mut bindings);
        b.finish()
    };
    let exact = render(bound);
    assert_eq!(exact.len(), tree.len());
    assert_eq!(exact.nodes(), tree.nodes());
    let mut short = bound;
    short.nodes -= 1;
    assert!(std::panic::catch_unwind(core::panic::AssertUnwindSafe(|| render(short))).is_err());
    let mut short = bound;
    short.text_bytes -= 1;
    assert!(std::panic::catch_unwind(core::panic::AssertUnwindSafe(|| render(short))).is_err());
    assert_eq!(value.resources.capacity(), 4);
}
fn native_owner(value: OwnerProject) -> Fixture {
    native_owner_at(value, crate::address::ProjectSection::Settings)
}
fn native_owner_at(value: OwnerProject, section: crate::address::ProjectSection) -> Fixture {
    use crate::{Event, Request, address::Address};
    let mut domain = Fixture::new(&limits());
    let out = native_drive(
        &mut domain,
        Event::Start {
            seed: 8,
            place: crate::address::Place::Jig(Address::Project { project: value.number, section }),
            records: List::with_capacity(0),
        },
    );
    native_drive(&mut domain, Served { served: true });
    let watch = out
        .iter()
        .find_map(|request| match request {
            Request::Client {
                asked: jig_client::Event::Watch { handle: watch, subject: jig_client::Watchable::Jig(Subject::Me) },
            } => Some(*watch),
            _ => None,
        })
        .unwrap();
    let out = native_drive(
        &mut domain,
        Watched { watch, event: WatchEvent::Delivered(Delivery::Me(Box::new(super::cards::document()))) },
    );
    let read = out
        .iter()
        .find_map(|request| match request {
            Request::Client {
                asked: jig_client::Event::Read { handle: read, read: jig_client::Readable::Jig(Read::Project { .. }) },
            } => Some(*read),
            _ => None,
        })
        .unwrap();
    native_drive(&mut domain, ReadPage { read, result: ReadResult::Value(ReadValue::OwnerProject(Box::new(value))) });
    domain
}
fn owner_page(domain: &Fixture) -> (Tree, JigBindings) {
    let w = words(false);
    let f = view::worst_case(&limits(), w).unwrap();
    let mut bound = capacity(f);
    bound.nodes = bound.nodes.checked_add(32).unwrap();
    bound.text_bytes = bound.text_bytes.checked_add(32_u32.checked_mul(bound.text_per_node).unwrap()).unwrap();
    let mut b = Builder::new(&bound);
    let mut bindings = JigBindings::new(f.bindings, 32);
    crate::pages::main(&mut b, &domain.view(), w, &mut bindings);
    let tree = b.finish();
    super::cards_access::check(&tree);
    (tree, bindings)
}
#[test]
#[expect(clippy::too_many_lines, reason = "complete explicit public service, pool and membership journey proof")]
fn owner_project_public_service_and_pool_inputs_decode_explicit_values_and_preserve_complete_membership_choices() {
    use crate::{Event, Request, action::Act};
    let mut value = source();
    value.number = 4;
    value.cut = false;
    value.policy.cut = false;
    value.acts = list(
        4,
        [ActKind::MakeService, ActKind::SetPool, ActKind::SetRoles].map(|act| Offered {
            effect: None,
            act,
            admission: Admission::Allowed,
        }),
    );
    let mut domain = native_owner(value.clone());
    let (tree, mut bindings) = owner_page(&domain);
    assert!(
        tree.nodes()
            .iter()
            .all(|node| node.role != Role::TextField || node.name.as_deref() != Some(b"Name".as_slice()))
    );
    let selected = tree
        .nodes()
        .iter()
        .filter(|node| node.role == Role::Button)
        .find_map(|node| {
            node.binding.and_then(|binding| match crate::decode(&mut bindings, binding.token, skein_ui::Act::Press) {
                Some(act @ Act::ProjectServiceRole { role: HeldRole::Policy { role: u32::MAX } }) => Some(act),
                _ => None,
            })
        })
        .unwrap();
    assert!(native_drive(&mut domain, Event::Act { action: selected }).is_empty());
    let (tree, mut bindings) = owner_page(&domain);
    let name = tree
        .nodes()
        .iter()
        .find(|node| node.role == Role::TextField && node.name.as_deref() == Some(b"Name".as_slice()))
        .unwrap();
    assert!(name.value.is_none());
    let act = crate::decode(&mut bindings, name.binding.unwrap().token, skein_ui::Act::Input { value: copy_of(b"") })
        .unwrap();
    let Act::ProjectSettings { ask } = &act else { panic!("explicit service request") };
    assert!(match ask.as_ref() {
        Ask::MakeService { project: 4, name, role: HeldRole::Policy { role: u32::MAX } } if name.bytes().is_empty() =>
            true,
        _ => false,
    });
    native_drive(&mut domain, Event::Act { action: act });
    let sent = native_drive(&mut domain, Event::Act { action: Act::Confirm });
    assert!(match &sent[..] {
        [Request::Client { asked: jig_client::Event::Send { request: jig_client::Ask::Jig(ask), .. } }]
            if match ask.as_ref() {
                Ask::MakeService { project: 4, role: HeldRole::Policy { role: u32::MAX }, name }
                    if name.bytes().is_empty() =>
                    true,
                _ => false,
            } =>
            true,
        _ => false,
    });
    let mut domain = native_owner(value.clone());
    let (tree, mut bindings) = owner_page(&domain);
    let pool = tree
        .nodes()
        .iter()
        .filter(|node| node.role == Role::TextField)
        .find_map(|node| {
            node.binding.and_then(|binding| {
                match crate::decode(
                    &mut bindings,
                    binding.token,
                    skein_ui::Act::Input { value: copy_of(b"18446744073709551615") },
                ) {
                    Some(Act::ProjectSettings { ask })
                        if match ask.as_ref() {
                            Ask::SetPool { project: 4, party: 0, budget: u64::MAX } => true,
                            _ => false,
                        } =>
                    {
                        Some((node, Act::ProjectSettings { ask }))
                    }
                    _ => None,
                }
            })
        })
        .unwrap();
    assert!(pool.0.value.is_none());
    assert!(has(&tree, b"No pool exists for this configured target"));
    native_drive(&mut domain, Event::Act { action: pool.1 });
    assert!(domain.project_page().settings.before.is_none());
    assert_eq!(domain.project_page().settings.pool_period, Some(0));
    let sent = native_drive(&mut domain, Event::Act { action: Act::Confirm });
    assert!(match &sent[..] {
        [Request::Client { asked: jig_client::Event::Send { request: jig_client::Ask::Jig(ask), .. } }]
            if match ask.as_ref() {
                Ask::SetPool { project: 4, party: 0, budget: u64::MAX } => true,
                _ => false,
            } =>
            true,
        _ => false,
    });
    let domain = native_owner(value.clone());
    let (tree, mut bindings) = owner_page(&domain);
    let chosen = tree
        .nodes()
        .iter()
        .filter(|node| node.role == Role::Button)
        .find_map(|node| {
            node.binding.and_then(|binding| match crate::decode(&mut bindings, binding.token, skein_ui::Act::Press) {
                Some(Act::ProjectSettings { ask })
                    if match ask.as_ref() {
                        Ask::SetRoles { holdings, .. }
                            if holdings.len() == 2 && holdings.get(0).unwrap().role == HeldRole::Observer =>
                        {
                            true
                        }
                        _ => false,
                    } =>
                {
                    Some(ask)
                }
                _ => None,
            })
        })
        .unwrap();
    let Ask::SetRoles { project, holdings } = chosen.as_ref() else { panic!("whole membership request") };
    assert_eq!(*project, 4);
    assert_eq!(holdings.capacity(), value.members.capacity());
    assert_eq!(holdings.get(0).unwrap().party, u64::MAX);
    assert_eq!(holdings.get(1).unwrap().party, 0);
    assert_eq!(holdings.get(1).unwrap().role, HeldRole::Owner);
}

#[test]
#[expect(clippy::too_many_lines, reason = "complete whole source fixture or ordered public journey proof")]
fn owner_project_public_goal_form_decodes_whole_bounded_procedure_and_keeps_proposed_review_before_one_keyed_request() {
    use crate::{Event, Request, action::Act, address::ProjectSection, fields::Purpose};
    let mut value = source();
    value.number = 4;
    let executor = OwnerExecutor::Procedure { connector: u16::MAX, code: u32::MAX };
    let recipient = Recipient::Roles { project: 4, roles: list(4, [u32::MAX, 0, u32::MAX]) };
    value.acts = list(
        4,
        [Offered {
            effect: None,
            act: ActKind::Reorder,
            admission: Admission::Bounded {
                within: Bounds::Create(CreateBounds {
                    authority: Box::new(crate::tests::cards_objects::owner_authority()),
                    pool_left: 17,
                    charters: list(4, [u32::MAX, 0, u32::MAX]),
                    procedures: list(4, [CreateProcedure { connector: u16::MAX, code: u32::MAX }]),
                    latest: None,
                    budget: 17,
                    maximum_budget: 23,
                    maximum_deadline: None,
                }),
                beyond: Beyond::Proposed { to: recipient.clone() },
            },
        }],
    );
    let mut domain = native_owner_at(value.clone(), ProjectSection::Goals);
    crate::tests::fixtures::set_creating(&mut domain, value.number, &value.acts.get(0).unwrap().admission);
    assert!(domain.project_page().document.is_none());
    let (tree, mut bindings) = owner_page(&domain);
    let choice = tree
        .nodes()
        .iter()
        .find_map(|node| {
            let act = crate::decode(&mut bindings, node.binding?.token, skein_ui::Act::Press)?;
            match act {
                Act::GoalForm { form } if form.executor == executor => Some(form),
                _ => None,
            }
        })
        .expect("supplied procedure choice");
    assert_eq!(choice.project, 4);
    assert_eq!(choice.budget, 17);
    assert!(native_drive(&mut domain, Event::Act { action: Act::GoalForm { form: choice } }).is_empty());
    let (tree, mut bindings) = owner_page(&domain);
    let words_field = tree.nodes().iter().find(|node| node.role == Role::TextArea).unwrap().binding.unwrap().token;
    let act =
        crate::decode(&mut bindings, words_field, skein_ui::Act::Input { value: copy_of(UTF8_TEXTS[0]) }).unwrap();
    assert!(match &act {
        Act::Edit { field: Purpose::NewGoal, text } if text.bytes() == UTF8_TEXTS[0] => true,
        _ => false,
    });
    native_drive(&mut domain, Event::Act { action: act });
    for (field, amount) in [
        (crate::sections::project::goals::Field::Budget, b"23".as_slice()),
        (crate::sections::project::goals::Field::Priority, b"4294967295".as_slice()),
    ] {
        let (tree, mut bindings) = owner_page(&domain);
        let form = domain.project_page().form.form.unwrap();
        let edited = crate::sections::project::goals::edit(form, field, amount).unwrap();
        let decoded = tree
            .nodes()
            .iter()
            .filter_map(|node| {
                crate::decode(&mut bindings, node.binding?.token, skein_ui::Act::Input { value: copy_of(amount) })
            })
            .find(|act| act == &edited)
            .unwrap();
        assert!(native_drive(&mut domain, Event::Act { action: decoded }).is_empty());
    }
    let (tree, mut bindings) = owner_page(&domain);
    let submit = tree
        .nodes()
        .iter()
        .filter_map(|node| crate::decode(&mut bindings, node.binding?.token, skein_ui::Act::Press))
        .find(|act| match act {
            Act::Submit { field: Purpose::NewGoal } => true,
            _ => false,
        })
        .unwrap();
    assert!(native_drive(&mut domain, Event::Act { action: submit }).is_empty());
    let projected = domain.project_page();
    let review = projected.form.confirmation.as_ref().unwrap();
    assert_eq!(review.form.executor, executor);
    assert_eq!(review.form.budget, 23);
    assert_eq!(review.form.priority, u32::MAX);
    assert_eq!(review.form.deadline, None);
    assert_eq!(review.words.bytes(), UTF8_TEXTS[0]);
    assert_eq!(review.pool_left, 17);
    assert!(review.proposed);
    assert_eq!(review.to.as_ref(), Some(&recipient));
    let sent = native_drive(&mut domain, Event::Act { action: Act::Confirm });
    let asks: Vec<_> = sent
        .iter()
        .filter_map(|row| match row {
            Request::Client { asked: jig_client::Event::Send { request: jig_client::Ask::Jig(ask), .. } } => {
                Some(ask.as_ref())
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        asks,
        [&crate::tests::fixtures::goal_request(
            crate::page::project::goals::GoalForm {
                project: 4,
                executor,
                budget: 23,
                priority: u32::MAX,
                deadline: None
            },
            Text::new(copy_of(UTF8_TEXTS[0]), limits().text).unwrap()
        )]
    );
    assert!(native_drive(&mut domain, Event::Act { action: Act::Confirm }).is_empty());
    assert_eq!(domain.project_page().owner.as_deref(), Some(&value));
}

#[test]
fn owner_project_public_offer_ceiling_preserves_twenty_five_in_order_and_rejects_reserved_twenty_six() {
    let w = words(false);
    let mut value = source();
    let original_requirement = view::required(&value, &limits(), w).unwrap();
    for populated in [0, 1, MAX_OFFERED_ACTS] {
        value.acts = List::with_capacity(MAX_OFFERED_ACTS);
        for index in 0..populated {
            value
                .acts
                .push(Offered {
                    effect: None,
                    act: if index % 2 == 0 { ActKind::SetPool } else { ActKind::ChangePolicy },
                    admission: Admission::Allowed,
                })
                .unwrap();
        }
        let original = value.clone();
        // Standalone facts and supported forms are already charged by their own envelope.
        assert_eq!(view::required(&value, &limits(), w), Some(original_requirement));
        let domain = native_owner(value.clone());
        assert_eq!(domain.project_page().owner.as_deref(), Some(&original));
        let (tree, _) = owner_page(&domain);
        assert!(!tree.is_empty());
        assert_eq!(value, original);
        value.acts = List::with_capacity(MAX_OFFERED_ACTS + 1);
        let oversized = value.clone();
        assert!(view::required(&value, &limits(), w).is_none());
        assert_eq!(value, oversized);
    }
    value.acts = List::with_capacity(MAX_OFFERED_ACTS);
    value
        .acts
        .push(Offered {
            effect: None,
            act: ActKind::Reorder,
            admission: Admission::Bounded {
                within: Bounds::Create(CreateBounds {
                    authority: Box::new(crate::tests::cards_objects::owner_authority()),
                    pool_left: 0,
                    charters: List::with_capacity(5),
                    procedures: List::with_capacity(4),
                    latest: None,
                    budget: 0,
                    maximum_budget: 0,
                    maximum_deadline: None,
                }),
                beyond: Beyond::Proposed { to: Recipient::Roles { project: u32::MAX, roles: List::with_capacity(4) } },
            },
        })
        .unwrap();
    assert!(view::required(&value, &limits(), w).is_none());
    value.acts = List::with_capacity(MAX_OFFERED_ACTS);
    value.members = List::with_capacity(5);
    assert!(view::required(&value, &limits(), w).is_none());
}

fn historical_old_spend(before: &Cursor, after: &Cursor) -> SpendPage {
    SpendPage {
        rows: list(
            4,
            [SpendPeriod {
                period: 997,
                budget: Budget {
                    total: 1061,
                    own: 1063,
                    ended: 1067,
                    reserved: 1069,
                    left: 1087,
                    funder: Actor::Task(3),
                },
                pools: List::with_capacity(4),
                tasks: List::with_capacity(4),
            }],
        ),
        before: Some(before.clone()),
        after: Some(after.clone()),
        cut: true,
    }
}
fn native_historical_project() -> (Fixture, OwnerSpendPage) {
    use crate::{
        Event, Request,
        action::Act,
        address::{Address, ProjectSection},
    };
    let mut value = source();
    value.number = 4;
    let mut domain = Fixture::new(&limits());
    let start = native_drive(
        &mut domain,
        Event::Start {
            seed: 8,
            place: crate::address::Place::Jig(Address::Project { project: 4, section: ProjectSection::Spend }),
            records: List::with_capacity(0),
        },
    );
    native_drive(&mut domain, Served { served: true });
    let watch = start
        .iter()
        .find_map(|request| match request {
            Request::Client {
                asked: jig_client::Event::Watch { handle: watch, subject: jig_client::Watchable::Jig(Subject::Me) },
            } => Some(*watch),
            _ => None,
        })
        .unwrap();
    let ready = native_drive(
        &mut domain,
        Watched { watch, event: WatchEvent::Delivered(Delivery::Me(Box::new(super::cards::document()))) },
    );
    let read =
        ready
            .iter()
            .find_map(|request| match request {
                Request::Client {
                    asked:
                        jig_client::Event::Read {
                            handle: read,
                            read: jig_client::Readable::Jig(Read::Project { project: 4 }),
                        },
                } => Some(*read),
                _ => None,
            })
            .unwrap();
    native_drive(&mut domain, ReadPage { read, result: ReadResult::Value(ReadValue::OwnerProject(Box::new(value))) });
    let read = ready
        .iter()
        .find_map(|request| match request {
            Request::Client {
                asked:
                    jig_client::Event::Read {
                        handle: read,
                        read: jig_client::Readable::Jig(Read::Spend { project: 4, after: None }),
                    },
            } => Some(*read),
            _ => None,
        })
        .unwrap();
    let before = Cursor::new(copy_of(UTF8_TEXTS[1]), 32).unwrap();
    let after = Cursor::new(copy_of(UTF8_TEXTS[2]), 32).unwrap();
    let old = historical_old_spend(&before, &after);
    native_drive(&mut domain, ReadPage { read, result: ReadResult::Value(ReadValue::Spend(Box::new(old))) });
    let requests = native_drive(
        &mut domain,
        Event::Act {
            action: Act::More { window: skein_lib::Token::new(crate::page::project::SPEND_WINDOW), end: End::After },
        },
    );
    let read = requests
        .iter()
        .find_map(|request| match request {
            Request::Client {
                asked:
                    jig_client::Event::Read {
                        handle: read,
                        read: jig_client::Readable::Jig(Read::Spend { project: 4, after: Some(cursor) }),
                    },
            } if cursor == &after => Some(*read),
            _ => None,
        })
        .unwrap();
    let page = historical_spend_page(before, after);
    native_drive(
        &mut domain,
        ReadPage { read, result: ReadResult::Value(ReadValue::OwnerSpend(Box::new(page.clone()))) },
    );
    (domain, page)
}
fn historical_project_tree(domain: &Fixture, w: &Words, bound: skein_ui::Limits) -> (Tree, JigBindings) {
    let mut b = Builder::new(&bound);
    let mut bindings = JigBindings::new(bound.bindings, 32);
    crate::pages::main(&mut b, &domain.view(), w, &mut bindings);
    (b.finish(), bindings)
}
fn historical_project_bound(w: &Words) -> skein_ui::Limits {
    let mut bound = capacity(view::worst_case(&limits(), w).unwrap());
    bound.nodes = bound.nodes.checked_add(32).unwrap();
    bound.text_bytes = bound.text_bytes.checked_add(32_u32.checked_mul(bound.text_per_node).unwrap()).unwrap();
    bound
}
#[test]
fn owner_project_genuine_spend_keeps_live_historical_and_earlier_period_facts_with_both_opaque_edges() {
    use crate::{Event, Request, action::Act};
    let (mut domain, page) = native_historical_project();
    assert_eq!(domain.project_page().owner_spend.as_deref(), Some(&page));
    assert_eq!(domain.project_page().spend.get(0).unwrap().period, 997);
    assert_eq!(domain.project_page().spend_before, page.before);
    assert_eq!(domain.project_page().spend_after, page.after);
    assert!(domain.project_page().spend_cut);
    let w = words(false);
    let (tree, mut bindings) = historical_project_tree(&domain, w, historical_project_bound(w));
    super::cards_access::check(&tree);
    for supplied in [b"103".as_slice(), b"977", b"979", b"981", b"983", b"985", b"997", b"1087"] {
        assert!(has(&tree, supplied), "missing {supplied:?}");
    }
    let position = |bytes: &[u8]| {
        tree.nodes().iter().position(|node| node.text.iter().any(|run| find(&run.text, bytes).is_some())).unwrap()
    };
    assert!(position(b"103") < position(b"977"));
    assert!(position(b"977") < position(b"997"));
    let edges: Vec<_> = tree
        .nodes()
        .iter()
        .filter_map(|node| node.binding)
        .filter_map(|binding| crate::decode(&mut bindings, binding.token, skein_ui::Act::Press))
        .filter(|act| match act {
            Act::More { window, .. } => *window == skein_lib::Token::new(22),
            _ => false,
        })
        .collect();
    assert_eq!(
        edges,
        [
            Act::More { window: skein_lib::Token::new(22), end: End::Before },
            Act::More { window: skein_lib::Token::new(22), end: End::After }
        ]
    );
    for (act, expected) in edges.into_iter().zip([page.before.clone().unwrap(), page.after.clone().unwrap()]) {
        let requests = native_drive(&mut domain, Event::Act { action: act });
        assert!(requests.iter().any(|request| match request {
            Request::Client {
                asked:
                    jig_client::Event::Read {
                        read: jig_client::Readable::Jig(Read::Spend { project: 4, after: Some(cursor) }),
                        ..
                    },
            } => cursor == &expected,
            _ => false,
        }));
    }
    assert_eq!(domain.project_page().owner_spend.as_deref(), Some(&page));
}
#[test]
#[expect(clippy::assertions_on_result_states, reason = "independent exact and one-under whole companion allocations")]
fn owner_project_historical_companion_prices_whole_languages_and_refuses_one_under_nodes_or_text() {
    let (domain, page) = native_historical_project();
    for second in [false, true] {
        let w = words(second);
        let bound = historical_project_bound(w);
        let (tree, _) = historical_project_tree(&domain, w, bound);
        assert!(tree.len() <= bound.nodes);
        let mut exact = bound;
        exact.nodes = tree.len();
        exact.text_bytes = tree.nodes().iter().map(crate::tests::text_bytes).sum();
        let (rebuilt, _) = historical_project_tree(&domain, w, exact);
        assert_eq!(rebuilt.nodes(), tree.nodes());
        for short in [
            skein_ui::Limits { nodes: exact.nodes - 1, ..exact },
            skein_ui::Limits { text_bytes: exact.text_bytes - 1, ..exact },
        ] {
            assert!(
                std::panic::catch_unwind(core::panic::AssertUnwindSafe(|| historical_project_tree(&domain, w, short)))
                    .is_err()
            );
        }
        let historical = crate::historical_pages::spend_required(&page, &limits(), w).unwrap();
        assert!(historical.nodes <= view::max_nodes(&limits()).unwrap());
        if second {
            assert!(has(&tree, b"second "));
        }
    }
    assert!(view::max_nodes(&crate::Limits { rows: u32::MAX, ..limits() }).is_none());
    assert_eq!(domain.project_page().owner_spend.as_deref(), Some(&page));
    assert_eq!(domain.project_page().spend.get(0).unwrap().period, 997);
}

const UTF8_TEXTS: [&[u8]; 6] = [b"~\0/!", b"~\0\x01", b"!\0\x02", b"~\0!", b"!\0~", &[b'~'; 32]];

pub(super) fn settings_tree() -> Tree {
    let mut value = source();
    value.number = 4;
    owner_page(&native_owner(value)).0
}

fn historical_spend_page(before: Cursor, after: Cursor) -> OwnerSpendPage {
    OwnerSpendPage {
        rows: list(
            4,
            [OwnerFundingRecord {
                period: Some(OwnerPeriodTimes { began: Wall::EPOCH, next: Some(Wall::from_nanos(u64::MAX)) }),
                funder: Funder::Period { project: 4, period: 777 },
                parent: None,
                numbers: OwnerNumbers { budget: 977, spent: 979, spent_below: 981, reserved: 983 },
                made: 985,
                closed: true,
            }],
        ),
        before: Some(before),
        after: Some(after),
        cut: true,
    }
}

#[test]
fn owner_project_header_and_tabs_show_the_supplied_name() {
    let domain = native_owner(source());
    let (tree, _) = owner_page(&domain);
    assert!(has(&tree, b"Project: Tally"));
    assert!(tree.nodes().iter().any(|node| node.role == Role::Tabs && node.name.as_deref() == Some(b"Tally")));
}
