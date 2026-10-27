use crate::{
    connector_values as render,
    connector_values_words::{self as words, Word},
};
use jig_client_test_domain::{
    connector_values::*,
    family::{Kind, Scope, Subject},
};
use jig_client_view::phrases::jig::JigPhrase;
use jig_client_view::{Owner, phrases};
use skein_ui::phrases::{Form as PhraseForm, HoleKind, PhraseLimits, Phrasebook, PluralClass, Segment};
use skein_ui::{Builder, Role, Tree};

use skein_lib::{List, Wall, bytes::copy_of};
fn list<T>(capacity: u32, values: impl IntoIterator<Item = T>) -> List<T> {
    let mut result = List::with_capacity(capacity);
    for value in values {
        result.push(value).ok().expect("fixture capacity");
    }
    result
}
fn path() -> Path {
    Path::new(list(2, [b"literal/slash".as_slice().into(), [0xff, 0x00].as_slice().into()]), 16).unwrap()
}
fn effect() -> Effect {
    Effect {
        kind: u16::MAX,
        resources: Box::new([path()]),
        purpose: u64::MAX,
        condition: Some(u64::MAX),
        target: u64::MAX,
        state: u64::MAX,
    }
}
fn description() -> Description {
    Description {
        kind: u16::MAX,
        resources: Box::new([path()]),
        purpose: u64::MAX,
        condition: true,
        form: Form::Transition,
        recovery: Recovery::Unrecoverable,
        price: Some(u64::MAX),
        state: u64::MAX,
    }
}
fn entry() -> OutboxEntry {
    OutboxEntry {
        number: u64::MAX,
        task: u64::MAX,
        key: Key {
            attempt: u64::MAX,
            completion: u32::MAX,
            position: u32::MAX,
            deployment: [u8::MAX; 16],
            task: u64::MAX,
            purpose: u64::MAX,
        },
        effect: effect(),
        attempt: Some(Attempt {
            number: u32::MAX,
            sent: Wall::from_nanos(u64::MAX),
            deadline: Wall::from_nanos(u64::MAX),
        }),
        phase: EffectPhase::Held,
    }
}
fn resource() -> ClientResource {
    ClientResource {
        path: path(),
        role: ResourceRole::Unavailable,
        hold: Hold::Pooled { mode: HoldMode::Wait },
        fact: Some(Fact { state: Some(u64::MAX), observed: Wall::from_nanos(u64::MAX), pending: true }),
    }
}
fn procedure() -> ProcedureState {
    ProcedureState {
        project: u32::MAX,
        task: u64::MAX,
        number: u16::MAX,
        resource: path(),
        index: u16::MAX,
        steps: u16::MAX,
        awaiting: true,
        delegate: None,
        revision: 0,
        deadline: Some(Wall::from_nanos(u64::MAX)),
    }
}
fn limits() -> ConnectorLimits {
    ConnectorLimits { rows: 3, paths: 2, segments: 2, segment_bytes: 16, bytes: u64::MAX }
}
fn document(value: ClientDocument, scope: Scope, words: &jig_client_view::words::Words) -> Tree {
    let value = ConnectorDocument { subject: Subject { scope, kind: Kind::Outbox(u64::MAX) }, value: Box::new(value) };
    let bound = render::requirements(&limits(), words).unwrap();
    let mut b = Builder::new(&bound);
    b.open(Role::Page);
    render::document(&mut b, &value, words);
    b.close();
    let tree = b.finish();
    assert!(tree.len() <= bound.nodes);
    tree
}
fn answer(value: ApplicationOutcome, words: &jig_client_view::words::Words) -> Tree {
    let bound = render::requirements(&limits(), words).unwrap();
    let mut b = Builder::new(&bound);
    b.open(Role::Page);
    render::answer(&mut b, &ConnectorAnswer { value }, words, limits().segment_bytes);
    b.close();
    let tree = b.finish();
    assert!(tree.len() <= render::answer_nodes().saturating_add(1));
    tree
}
fn contains(tree: &Tree, value: &[u8]) -> bool {
    tree.nodes().iter().any(|node| {
        node.name.as_ref().is_some_and(|name| name.windows(value.len()).any(|part| part == value))
            || node.text.iter().any(|run| run.text.windows(value.len()).any(|part| part == value))
    })
}
fn expect(tree: &Tree, values: &[&[u8]]) {
    for value in values {
        assert!(contains(tree, value), "missing {}", String::from_utf8_lossy(value));
    }
}
#[test]
fn connector_values_view_renders_all_eight_complete_documents_at_maximum_scalar_width() {
    let words = words::english(64).unwrap();
    let max = b"18446744073709551615".as_slice();
    let values = [
        (
            ClientDocument::Object { object: u64::MAX, now: ApplicationObject { revision: u64::MAX, value: u64::MAX } },
            b"Owner object".as_slice(),
        ),
        (
            ClientDocument::Panel {
                task: u64::MAX,
                procedure: Some(procedure()),
                result: Some(u16::MAX),
                resources: list(3, [resource()]),
                effects: list(3, [entry()]),
            },
            b"Procedure panel",
        ),
        (ClientDocument::Result { task: u64::MAX, code: u16::MAX }, b"Procedure result"),
        (
            ClientDocument::Effect { proposal: u64::MAX, task: u64::MAX, effect: effect(), description: description() },
            b"Proposed effect",
        ),
        (ClientDocument::Outbox { entry: entry(), description: description() }, b"Owner outbox entry"),
        (
            ClientDocument::Hold { task: u64::MAX, resources: list(3, [resource()]), effects: list(3, [entry()]) },
            b"Hold account",
        ),
        (ClientDocument::Project { project: u32::MAX, resources: list(3, [resource()]) }, b"Owner project"),
        (
            ClientDocument::Deployment {
                outbox: list(3, [entry()]),
                subject_limit: u32::MAX,
                staged_limit: u32::MAX,
                request_budget: DiagnosticAvailability::NotKept,
                system_last_response: DiagnosticAvailability::NotKept,
            },
            b"Owner deployment",
        ),
    ];
    for (index, (value, title)) in values.into_iter().enumerate() {
        let tree = document(value, Scope::Project(u32::MAX), &words);
        expect(&tree, &[title, b"Project scope: 4294967295"]);
        if index != 6 {
            expect(&tree, &[max]);
        }
        if index != 0 {
            assert!(!contains(&tree, b"Object revision:"), "parts have no revision");
        }
        if index == 1 {
            expect(
                &tree,
                &[
                    b"Procedure program: 65535",
                    b"Next procedure action: 65535",
                    b"Committed procedure steps: 65535",
                    b"Result code: 65535",
                    b"Absolute deadline: 18446744073709551615",
                    b"Fact observed at: 18446744073709551615",
                    b"Prepared attempt: 4294967295",
                ],
            );
        }
        if [1, 3, 4, 5, 6, 7].contains(&index) {
            expect(&tree, &[b"literal/slash", b"\\xFF\\x00"]);
        }
        for node in tree.nodes() {
            assert!(node.binding.is_none() && node.link.is_none(), "values supply no mutation permission");
        }
    }
}
#[test]
fn connector_values_view_preserves_literal_segment_boundaries_and_each_outbox_scope() {
    let words = words::english(64).unwrap();
    for scope in [Scope::Project(0), Scope::Project(3), Scope::Project(4), Scope::Deployment] {
        let tree = document(ClientDocument::Outbox { entry: entry(), description: description() }, scope, &words);
        let subject = Subject { scope, kind: Kind::Outbox(u64::MAX) };
        assert!(tree.nodes().iter().any(|node| node.key == Some(crate::family_key(&subject))));
        expect(&tree, &[b"Entry: 18446744073709551615", b"Key completion: 4294967295", b"Key position: 4294967295"]);
        let literal = tree
            .nodes()
            .iter()
            .flat_map(|node| node.text.iter())
            .filter(|run| run.text.as_ref() == b"literal/slash")
            .count();
        assert_eq!(literal, 2, "request and description own separate complete paths");
        assert!(!contains(&tree, b"Owner proposal reference:"));
        assert!(tree.nodes().iter().any(|node| {
            node.key
                .is_some_and(|key| key.owner == (Owner::Application { number: jig_client_test_domain::FIRST }).opaque())
        }));
    }
}
#[test]
fn connector_values_view_all_optional_fields_keep_absence_distinct_from_zero() {
    let words = words::english(64).unwrap();
    let tree = document(
        ClientDocument::Panel {
            task: 0,
            procedure: None,
            result: None,
            resources: list(
                1,
                [ClientResource { path: path(), role: ResourceRole::Owned, hold: Hold::None, fact: None }],
            ),
            effects: list(0, []),
        },
        Scope::Project(0),
        &words,
    );
    expect(&tree, &[b"Procedure not supplied", b"Result not supplied", b"Fact not supplied"]);
    for present in [false, true] {
        let mut procedure = procedure();
        procedure.awaiting = present;
        procedure.deadline = if present { Some(Wall::EPOCH) } else { None };
        let mut resource = resource();
        resource.fact =
            Some(Fact { state: if present { Some(0) } else { None }, observed: Wall::EPOCH, pending: present });
        let mut entry = entry();
        entry.effect.condition = if present { Some(0) } else { None };
        entry.attempt =
            if present { Some(Attempt { number: 0, sent: Wall::EPOCH, deadline: Wall::EPOCH }) } else { None };
        let tree = document(
            ClientDocument::Panel {
                task: 0,
                procedure: Some(procedure),
                result: Some(0),
                resources: list(1, [resource]),
                effects: list(1, [entry]),
            },
            Scope::Deployment,
            &words,
        );
        expect(&tree, &[b"Result code: 0"]);
        if present {
            expect(
                &tree,
                &[
                    b"Resource",
                    b"Absolute deadline: 0",
                    b"Starting state: 0",
                    b"Prepared attempt: 0",
                    b"Observed resource state: 0",
                    b"Fact pending",
                    b"Procedure awaiting an answer",
                ],
            );
        } else {
            expect(
                &tree,
                &[
                    b"No procedure deadline",
                    b"No starting condition",
                    b"No prepared attempt",
                    b"Resource state absent",
                    b"Fact final",
                    b"Procedure not awaiting an answer",
                ],
            );
        }
    }
    let mut facts = description();
    facts.condition = false;
    facts.price = None;
    let tree = document(
        ClientDocument::Effect { proposal: 0, task: 0, effect: effect(), description: facts },
        Scope::Project(0),
        &words,
    );
    expect(
        &tree,
        &[b"Maximum price not supplied", b"Description has no starting condition", b"Owner proposal reference: 0"],
    );
}
#[test]
fn connector_values_view_every_effect_phase_recovery_form_role_and_hold_remains_distinct() {
    let words = words::english(64).unwrap();
    for (phase, label) in [
        (EffectPhase::Kept, b"Effect kept".as_slice()),
        (EffectPhase::Sent, b"Effect sent"),
        (EffectPhase::Uncertain, b"Effect uncertain"),
        (EffectPhase::Retry, b"Effect retry"),
        (EffectPhase::Repeat, b"Effect repeat"),
        (EffectPhase::Held, b"Effect held"),
    ] {
        let mut value = entry();
        value.phase = phase;
        expect(
            &document(ClientDocument::Outbox { entry: value, description: description() }, Scope::Deployment, &words),
            &[label],
        );
    }
    for (form, label) in
        [(Form::Creation, b"Creation".as_slice()), (Form::Transition, b"Transition"), (Form::Set, b"Set")]
    {
        for (recovery, recovery_label) in [
            (Recovery::Keyed, b"Keyed recovery".as_slice()),
            (Recovery::Conditional, b"Conditional recovery"),
            (Recovery::Idempotent, b"Idempotent recovery"),
            (Recovery::Unrecoverable, b"Unrecoverable"),
        ] {
            let mut value = description();
            value.form = form;
            value.recovery = recovery;
            expect(
                &document(
                    ClientDocument::Effect { proposal: 1, task: 2, effect: effect(), description: value },
                    Scope::Project(3),
                    &words,
                ),
                &[label, recovery_label],
            );
        }
    }
    for (role, label) in [
        (ResourceRole::Owned, b"Owned resource".as_slice()),
        (ResourceRole::Participating, b"Participating resource"),
        (ResourceRole::Context, b"Context resource"),
        (ResourceRole::Unavailable, b"Unavailable resource"),
    ] {
        for (hold, hold_label) in [
            (Hold::Exclusive { mode: HoldMode::Refuse }, b"Exclusive hold, refuse when taken".as_slice()),
            (Hold::Exclusive { mode: HoldMode::Wait }, b"Exclusive hold, wait when taken"),
            (Hold::Pooled { mode: HoldMode::Refuse }, b"Pooled hold, refuse when taken"),
            (Hold::Pooled { mode: HoldMode::Wait }, b"Pooled hold, wait when taken"),
            (Hold::None, b"No resource hold"),
        ] {
            let mut value = resource();
            value.role = role;
            value.hold = hold;
            expect(
                &document(
                    ClientDocument::Project { project: 3, resources: list(1, [value]) },
                    Scope::Project(3),
                    &words,
                ),
                &[label, hold_label],
            );
        }
    }
}
#[test]
fn connector_values_view_every_owner_terminal_retains_changed_moved_values_and_exact_role() {
    let words = words::english(64).unwrap();
    for (value, label) in [
        (
            ApplicationOutcome::Changed { object: 7, now: ApplicationObject { revision: 8, value: 9 } },
            b"Object changed".as_slice(),
        ),
        (ApplicationOutcome::Moved { object: 7, now: ApplicationObject { revision: 8, value: 9 } }, b"Object moved"),
    ] {
        expect(&answer(value, &words), &[label, b"Object: 7", b"Object revision: 8", b"Value: 9"]);
    }
    for (value, label) in [
        (ApplicationOutcome::Role { held: u32::MAX }, b"Held connector role: 4294967295".as_slice()),
        (ApplicationOutcome::Unknown, b"Unknown owner object"),
        (ApplicationOutcome::Exhausted, b"Owner exhausted"),
    ] {
        expect(&answer(value, &words), &[label]);
    }
}
#[test]
fn connector_values_view_selected_checked_language_covers_owner_values_and_reordered_refusal_holes() {
    let normal = words::language(phrases::english::language());
    let mut selected = words::language(phrases::english::language());
    for phrase in &mut selected.phrases {
        if phrase.id == Word::Value.id() {
            phrase.forms = Box::new([PhraseForm {
                class: PluralClass::Other,
                segments: Box::new([
                    Segment::Hole { index: 0, kind: HoleKind::Number },
                    Segment::Text(copy_of(b" valeur choisie")),
                ]),
            }]);
        }
        if phrase.id == JigPhrase::BudgetMissing.id() {
            phrase.forms = Box::new([PhraseForm {
                class: PluralClass::Other,
                segments: Box::new([
                    Segment::Hole { index: 1, kind: HoleKind::Amount },
                    Segment::Text(copy_of(b" requis, ")),
                    Segment::Hole { index: 0, kind: HoleKind::Amount },
                    Segment::Text(copy_of(b" disponibles")),
                ]),
            }]);
        }
    }
    let count = u32::try_from(normal.phrases.len()).unwrap();
    let mut segments = 0_u32;
    let mut text_bytes = 0_u32;
    for language in [&normal, &selected] {
        for phrase in &language.phrases {
            for form in &phrase.forms {
                segments = segments.checked_add(u32::try_from(form.segments.len()).unwrap()).unwrap();
                for segment in &form.segments {
                    if let Segment::Text(bytes) = segment {
                        text_bytes = text_bytes.checked_add(u32::try_from(bytes.len()).unwrap()).unwrap();
                    }
                }
            }
        }
    }
    let book = Phrasebook::load(
        words::shapes(phrases::jig::shapes(64)),
        Box::new([normal, selected]),
        &PhraseLimits { languages: 2, phrases: count, holes: 4, segments, text_bytes, hole_bytes: 64 },
    )
    .unwrap();
    let mut words = jig_client_view::words::Words::new(book, 1, 0);
    let tree = document(
        ClientDocument::Object { object: 1, now: ApplicationObject { revision: 2, value: 3 } },
        Scope::Project(4),
        &words,
    );
    expect(&tree, &[b"3 valeur choisie"]);
    assert!(!contains(&tree, b"Value: 3"));
    let value = ApplicationOutcome::CoreRefused {
        why: application_core::Refusal::Authority {
            lacking: application_core::Lack::Budget { needs: 19, left: 7 },
            proposable: None,
        },
    };
    expect(&answer(value, &words), &[b"19 requis, 7 disponibles", b"Proposal eligibility not supplied"]);
    words.set_language(0);
    expect(
        &document(
            ClientDocument::Object { object: 1, now: ApplicationObject { revision: 2, value: 3 } },
            Scope::Project(4),
            &words,
        ),
        &[b"Value: 3"],
    );
    let mut invalid = words::language(phrases::english::language());
    for phrase in &mut invalid.phrases {
        if phrase.id == Word::Value.id() {
            phrase.forms = Box::new([PhraseForm {
                class: PluralClass::Other,
                segments: Box::new([Segment::Hole { index: 0, kind: HoleKind::Name }]),
            }]);
        }
    }
    Phrasebook::load(
        words::shapes(phrases::jig::shapes(64)),
        Box::new([invalid]),
        &PhraseLimits { languages: 1, phrases: count, holes: 4, segments: 3000, text_bytes: 30000, hole_bytes: 64 },
    )
    .unwrap_err();
}
#[test]
fn connector_values_composed_whole_languages_admit_exact_segments_and_refuse_one_under() {
    let normal = words::language(phrases::english::language());
    let mut segments = 0_u32;
    let mut text_bytes = 0_u32;
    for phrase in &normal.phrases {
        for form in &phrase.forms {
            segments = segments.checked_add(u32::try_from(form.segments.len()).unwrap()).unwrap();
            for segment in &form.segments {
                if let Segment::Text(bytes) = segment {
                    text_bytes = text_bytes.checked_add(u32::try_from(bytes.len()).unwrap()).unwrap();
                }
            }
        }
    }
    let count = u32::try_from(normal.phrases.len()).unwrap();
    let shapes = words::shapes(phrases::jig::shapes(64));
    let languages = Box::new([normal.clone(), normal]);
    let limits = PhraseLimits {
        languages: 2,
        phrases: count,
        holes: 4,
        segments: segments.checked_mul(2).unwrap(),
        text_bytes: text_bytes.checked_mul(2).unwrap(),
        hole_bytes: 64,
    };
    let mut short = limits;
    short.segments = short.segments.checked_sub(1).unwrap();
    assert_eq!(
        Phrasebook::load(shapes.clone(), languages.clone(), &short).unwrap_err(),
        skein_ui::phrases::Invalid::Limit
    );
    for admitted in [limits.segments, limits.segments.checked_add(1).unwrap()] {
        let mut capacity = limits;
        capacity.segments = admitted;
        let book = Phrasebook::load(shapes.clone(), languages.clone(), &capacity).unwrap();
        for language in 0..2 {
            assert_eq!(
                book.say(language, Word::Value.id(), &[skein_ui::phrases::Hole::Number(3)]).unwrap().text.as_ref(),
                b"Value: 3"
            );
        }
    }
    let mut short_holes = limits;
    short_holes.holes = 3;
    assert_eq!(Phrasebook::load(shapes, languages, &short_holes).unwrap_err(), skein_ui::phrases::Invalid::Limit);
}
#[test]
fn connector_values_view_the_capacity_envelope_covers_full_rows_paths_and_escaped_segments() {
    let words = words::english(64).unwrap();
    let full_path = || Path::new(list(2, [Box::<[u8]>::from([0xff; 16]), Box::<[u8]>::from([0xff; 16])]), 16).unwrap();
    let mut resources = List::with_capacity(3);
    let mut effects = List::with_capacity(3);
    for _ in 0_u32..3 {
        let mut value = resource();
        value.path = full_path();
        resources.push(value).unwrap();
        let mut value = entry();
        value.effect.resources = Box::new([full_path(), full_path()]);
        effects.push(value).unwrap();
    }
    let mut procedure = procedure();
    procedure.resource = full_path();
    let tree = document(
        ClientDocument::Panel { task: 0, procedure: Some(procedure), result: Some(0), resources, effects },
        Scope::Project(3),
        &words,
    );
    assert_eq!(tree.len(), render::document_nodes(&limits()).unwrap() + 1, "full panel reaches every counted node");
    let bound = render::requirements(&limits(), &words).unwrap();
    assert!(jig_client_view::limits::jig_worst_case(&bound).is_some());
    assert!(render::worst_case(&limits(), &words).is_some());
    let mut overflow = limits();
    overflow.rows = u32::MAX;
    assert!(render::document_nodes(&overflow).is_none());
    overflow = limits();
    overflow.segment_bytes = u32::MAX;
    assert!(render::requirements(&overflow, &words).is_none());
    assert!(words.book().capacity(Word::Value.id()).unwrap() <= bound.text_per_node);
}
#[test]
fn connector_values_view_exact_core_refusals_cover_all_admission_categories_and_eligibility_states() {
    use application_core::{Lack, LimitKind, Refusal};
    let words = words::english(64).unwrap();
    for (which, label) in [
        (LimitKind::Words, "Engine admission limit: words"),
        (LimitKind::Identity, "Engine admission limit: identity"),
        (LimitKind::Roles, "Engine admission limit: roles"),
        (LimitKind::Arithmetic, "Engine admission limit: arithmetic"),
        (LimitKind::Duplicate, "Engine admission limit: duplicate"),
        (LimitKind::Empty, "Engine admission limit: empty"),
        (LimitKind::Batch, "Engine admission limit: batch"),
        (LimitKind::ProjectLiveTasks, "Engine admission limit: project live tasks"),
        (LimitKind::LiveTasks, "Engine admission limit: live tasks"),
        (LimitKind::Tree, "Engine admission limit: tree"),
        (LimitKind::Depth, "Engine admission limit: depth"),
        (LimitKind::Delegates, "Engine admission limit: delegates"),
        (LimitKind::Subscription, "Engine admission limit: subscription"),
        (LimitKind::Dependencies, "Engine admission limit: dependencies"),
        (LimitKind::Cycle, "Engine admission limit: cycle"),
        (LimitKind::Spec, "Engine admission limit: spec"),
        (LimitKind::Contract, "Engine admission limit: contract"),
        (LimitKind::Authority, "Engine admission limit: authority"),
        (LimitKind::Inputs, "Engine admission limit: inputs"),
        (LimitKind::Attempts, "Engine admission limit: attempts"),
        (LimitKind::LiveDelegates, "Engine admission limit: live delegates"),
        (LimitKind::Restore, "Engine admission limit: restore"),
        (LimitKind::Read, "Engine admission limit: read"),
        (LimitKind::Turn, "Engine admission limit: turn"),
        (LimitKind::HoldKind, "Engine admission limit: hold kind"),
        (LimitKind::HoldTaken, "Engine admission limit: hold taken"),
        (LimitKind::Holds, "Engine admission limit: holds"),
        (LimitKind::InboxFull, "Engine admission limit: inbox full"),
        (LimitKind::Notes, "Engine admission limit: notes"),
        (LimitKind::Revision, "Engine admission limit: revision"),
        (LimitKind::Question, "Engine admission limit: question"),
        (LimitKind::Priority, "Engine admission limit: priority"),
        (LimitKind::Policy, "Engine admission limit: policy"),
        (LimitKind::Resource, "Engine admission limit: resource"),
        (LimitKind::Service, "Engine admission limit: service"),
    ] {
        expect(&answer(ApplicationOutcome::CoreRefused { why: Refusal::Limit { which } }, &words), &[label.as_bytes()]);
    }
    for (proposable, label) in [
        (None, b"Proposal eligibility not supplied".as_slice()),
        (Some(false), b"Proposal not permitted"),
        (Some(true), b"Proposal permitted"),
    ] {
        expect(
            &answer(
                ApplicationOutcome::CoreRefused {
                    why: Refusal::Authority { lacking: Lack::Budget { needs: u64::MAX, left: 7 }, proposable },
                },
                &words,
            ),
            &[b"Your pool has 7 left; this needs 18446744073709551615", label],
        );
    }
    assert_eq!(crate::connector_values_refusal::refusal(Refusal::Role { held: Some(u32::MAX) }), None);
    assert_eq!(crate::connector_values_refusal::refusal(Refusal::Role { held: None }), None);
    for (why, label) in [
        (Refusal::Role { held: None }, b"No held role was supplied".as_slice()),
        (Refusal::Role { held: Some(u32::MAX) }, b"Held connector role: 4294967295"),
        (Refusal::Taken { by: u64::MAX }, b"Held by party 18446744073709551615"),
        (Refusal::NotWaiting, b"This object waits for someone else"),
        (Refusal::NoFurther, b"This holder cannot pass further"),
        (Refusal::Unknown, b"No visible object"),
        (Refusal::NotOffered, b"not offered"),
        (Refusal::KeyConflict, b"key"),
    ] {
        expect(&answer(ApplicationOutcome::CoreRefused { why }, &words), &[label]);
    }
}
#[test]
fn connector_values_view_all_sparse_authority_categories_keep_absent_requested_facts_absent() {
    use application_core::{Lack, Refusal};
    use jig_client_domain::contract::{Lacking, Refusal as ClientRefusal};
    let words = words::english(64).unwrap();
    let cases = [
        (Lack::Approver, b"Proposal approval authority is missing".as_slice()),
        (Lack::Again { required: true }, b"choice is required"),
        (Lack::Again { required: false }, b"choice was unexpected"),
        (Lack::Grant { kind: u16::MAX }, b"operation kind 65535"),
        (Lack::Funding, b"funding"),
        (Lack::TaskKind, b"Executor kind not allowed"),
        (Lack::Depth { most: u32::MAX }, b"Permitted descendant depth: 4294967295"),
        (Lack::Deadline, b"deadline"),
        (Lack::Tasks { left: u32::MAX }, b"Lifetime tasks left: 4294967295"),
        (Lack::Tool, b"tool family"),
        (Lack::Scope, b"note scope"),
        (Lack::Models, b"requested model"),
        (Lack::Reference, b"verified reference"),
        (Lack::ResourceAccess, b"resource access"),
        (Lack::ResourceUnavailable, b"unavailable"),
        (Lack::Shape, b"shape"),
        (Lack::Account, b"account"),
        (Lack::Writer, b"held"),
        (Lack::Required { connector: u16::MAX, kind: u16::MAX }, b"65535"),
        (Lack::Failed { connector: u16::MAX, kind: u16::MAX }, b"65535"),
        (Lack::Unguarded { connector: u16::MAX, kind: u16::MAX }, b"65535"),
    ];
    for (lacking, label) in cases {
        let why = Refusal::Authority { lacking, proposable: None };
        let tree = answer(ApplicationOutcome::CoreRefused { why }, &words);
        expect(&tree, &[label]);
        assert!(!contains(&tree, b"Requires effect kind"), "a raw kind carries no invented connector/path grant");
        assert!(!contains(&tree, b"Needs 0 tasks"));
    }
    assert_eq!(
        crate::connector_values_refusal::refusal(Refusal::Authority {
            lacking: Lack::Tasks { left: 9 },
            proposable: None
        }),
        Some(ClientRefusal::Authority { lacking: Lacking::Tasks { needed: None, left: 9 }, proposable: None })
    );
    assert_eq!(
        crate::connector_values_refusal::refusal(Refusal::Authority {
            lacking: Lack::Deadline,
            proposable: Some(true)
        }),
        Some(ClientRefusal::Authority {
            lacking: Lacking::Deadline { asked: None, latest: None },
            proposable: Some(true)
        })
    );
}
#[test]
fn connector_values_view_every_moved_hold_reason_failure_and_ending_keeps_only_actual_facts() {
    use application_core::{Ending, FailureClass, HoldReason, Moved, Party, Refusal};
    let words = words::english(64).unwrap();
    for (reason, label) in [
        (HoldReason::Stalled, b"stopped progressing".as_slice()),
        (HoldReason::Dependency { task: u64::MAX }, b"Dependency task 18446744073709551615"),
        (HoldReason::Stopped, b"Task stopped"),
        (HoldReason::Placed { hold: u64::MAX }, b"Held by a placed hold"),
        (HoldReason::Drift, b"resource drifted"),
        (HoldReason::Effects, b"Effects need a decision"),
        (HoldReason::EffectFailed, b"effect failed permanently"),
        (HoldReason::Uncertain { entry: u64::MAX }, b"Uncertain outbox entry 18446744073709551615"),
        (HoldReason::Budget, b"activation budget"),
        (HoldReason::Deadline, b"Deadline passed"),
        (HoldReason::HoldsWaited, b"holds waited too long"),
        (HoldReason::ResourceUnavailable, b"unavailable"),
        (HoldReason::ResourceAccess, b"resource access"),
        (HoldReason::PoolLost { pool: u64::MAX }, b"pool 18446744073709551615"),
        (HoldReason::Procedure, b"Procedure needs a decision"),
    ] {
        expect(
            &answer(
                ApplicationOutcome::CoreRefused {
                    why: Refusal::Moved { now: Moved::Hold { revision: u64::MAX, reason } },
                },
                &words,
            ),
            &[label, b"18446744073709551615"],
        );
    }
    for (class, label) in [
        (FailureClass::Transient, b"Temporary failure".as_slice()),
        (FailureClass::Permanent, b"Permanent failure"),
        (FailureClass::Run, b"Executor reported failure"),
        (FailureClass::Agent, b"Agent failed"),
        (FailureClass::Lost, b"Host lost"),
        (FailureClass::Invalid, b"Result broke its contract"),
    ] {
        let tree = answer(
            ApplicationOutcome::CoreRefused {
                why: Refusal::Moved { now: Moved::Hold { revision: 3, reason: HoldReason::Failures { class } } },
            },
            &words,
        );
        assert!(!contains(&tree, b" at 0"));
        expect(&tree, &[b"Current revision 3", label]);
    }
    for now in
        [Moved::Spec { revision: u64::MAX }, Moved::Person { revision: u64::MAX }, Moved::Note { revision: u32::MAX }]
    {
        let tree = answer(ApplicationOutcome::CoreRefused { why: Refusal::Moved { now } }, &words);
        expect(&tree, &[b"Current revision"]);
    }
    for (how, label) in [
        (Ending::Done, b"Completed".as_slice()),
        (Ending::Failed, b"Failed"),
        (Ending::Deleted, b"Deleted"),
        (Ending::Decided { by: u64::MAX, at: Wall::from_nanos(u64::MAX) }, b"By party 18446744073709551615"),
    ] {
        expect(&answer(ApplicationOutcome::CoreRefused { why: Refusal::Ended { how } }, &words), &[label]);
    }
    for (by, label) in [
        (Party::Person(u64::MAX), b"By party 18446744073709551615".as_slice()),
        (Party::Task(u64::MAX), b"By task 18446744073709551615"),
        (Party::Deployment { project: u32::MAX }, b"By deployment in project 4294967295"),
    ] {
        expect(
            &answer(
                ApplicationOutcome::CoreRefused {
                    why: Refusal::Ended { how: Ending::Cancelled { by, at: Wall::from_nanos(u64::MAX) } },
                },
                &words,
            ),
            &[b"Cancelled", label, b"18446744073709551615"],
        );
    }
}

mod deployment_diagnostics;
