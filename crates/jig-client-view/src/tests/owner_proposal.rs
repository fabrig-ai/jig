#![expect(
    clippy::disallowed_methods,
    clippy::wildcard_enum_match_arm,
    reason = "ordinary proofs query checked source trees and select public action requests"
)]
use crate::tests::fixtures::Fixture;
use crate::words::Words;
use crate::{JigBindings, owner_proposal as view};
use skein_ui::phrases::{Hole, PhraseLimits, Phrasebook, Segment, SpanUnit, WallFormat};
use skein_ui::{Builder, Tree};

use jig_client::contract::*;
use skein_lib::{
    List, Wall,
    bytes::{copy_of, find},
};
fn doc(bytes: &[u8], cut: bool) -> DocumentText {
    DocumentText { bytes: Text::new(copy_of(bytes), 32).unwrap(), cut }
}
fn limits() -> crate::Limits {
    crate::Limits { rows: 3, text: 32, ..super::cards::CLIENT }
}
fn part() -> OwnerPart {
    let mut segments = List::with_capacity(3);
    for s in [b"a/b".as_slice(), &[255, 0, 254], b""] {
        segments.push(copy_of(s)).unwrap();
    }
    OwnerPart { connector: u16::MAX, kind: OwnerPartKind::Outbox, key: OwnerPartKey::Path(segments) }
}
fn authority() -> OwnerAuthorityDocument {
    let mut grants = List::with_capacity(3);
    let mut notes = List::with_capacity(3);
    for _ in 0_u32..3 {
        let mut segments = List::with_capacity(3);
        for _ in 0_u32..3 {
            segments.push(copy_of(&[255; 32])).unwrap();
        }
        let pattern = OwnerPattern { segments, last: OwnerPatternEnd::Open(copy_of(&[0; 32])) };
        grants.push(OwnerGrant { connector: u16::MAX, kind: u16::MAX, pattern: pattern.clone() }).unwrap();
        notes.push(OwnerResourceScope { connector: u16::MAX, pattern }).unwrap();
    }
    let mut kinds = List::with_capacity(3);
    for v in [
        OwnerAuthorityExecutor::Charter(u32::MAX),
        OwnerAuthorityExecutor::Procedure(u32::MAX),
        OwnerAuthorityExecutor::Role(u32::MAX),
    ] {
        kinds.push(v).unwrap();
    }
    OwnerAuthorityDocument {
        authority: OwnerAuthority {
            models: None,
            tools: u64::MAX,
            grants,
            delegation: OwnerDelegation { kinds, tasks: u32::MAX, depth: u32::MAX },
            spend: u64::MAX,
            deadline: Some(Wall::from_nanos(u64::MAX)),
            notes: u8::MAX,
            note_resources: notes,
        },
        cut: true,
    }
}
fn state(words: &[u8], cut: bool) -> OwnerAmendmentState {
    let mut parameters = List::with_capacity(3);
    let mut inputs = List::with_capacity(3);
    let mut dependencies = List::with_capacity(3);
    for n in 0..3 {
        parameters.push(ParameterDocument::Bytes { name: n, value: doc(UTF8_TEXTS[0], true) }).unwrap();
        inputs.push(u64::MAX.checked_sub(u64::from(n)).unwrap()).unwrap();
        dependencies.push(u64::from(n)).unwrap();
    }
    OwnerAmendmentState {
        addressed: Some(PersonAddress::Role(u32::MAX)),
        taken_by: Some(u64::MAX),
        instructions: Some(doc(words, cut)),
        spec: SpecificationDocument {
            specification: DocumentSpecification {
                asks: false,
                words: doc(b"# supplied spec", true),
                parameters,
                inputs,
            },
            cut: true,
        },
        authority: authority(),
        wake: super::cards_objects::task_doc(&[]).wake,
        dependencies,
        cut: true,
    }
}
fn hold() -> OwnerChipHold {
    OwnerChipHold {
        placed: None,
        circle: None,
        account: None,
        source: Some(part()),
        reason: HoldCategory::Uncertain { entry: u64::MAX },
        revision: u64::MAX,
        before: OwnerPriorPhase::Active(RunStage::Running { attempt: 17 }),
        escalation: Some(OwnerHoldEscalation::Rejected { by: 19, reason: doc(b"hold explanation", true) }),
        tries: OwnerFailureTries { transient: 1, permanent: 2, run: 3, agent: 4, lost: 5, invalid: 6 },
        hold_wait_since: Some(Wall::from_nanos(23)),
    }
}
fn source(action: OwnerProposalAction) -> OwnerProposal {
    OwnerProposal {
        key: ProposalKey { proposer: Proposer::Party(u64::MAX), number: u64::MAX },
        project: u32::MAX,
        reason: doc(b"# whole proposal reason", true),
        action,
        as_holder: true,
        waiting: OwnerProposalWaiting { holder: OwnerHistoryHolder::Task(31), since: Wall::from_nanos(37) },
        funding: List::with_capacity(3),
        acts: List::with_capacity(3),
    }
}
fn words(selected: bool) -> Words {
    let mut first = crate::phrases::english::language();
    first.wall = WallFormat::Nanos;
    first.span_unit = SpanUnit::Nanos;
    let languages: Box<[skein_ui::phrases::Language]> = if selected {
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
        Box::new([first, second])
    } else {
        Box::new([first])
    };
    Words::new(
        Phrasebook::load(
            crate::phrases::jig::shapes(256),
            languages,
            &PhraseLimits {
                languages: 2,
                phrases: crate::owner_words::phrase_count(),
                holes: 4,
                segments: 10_000,
                text_bytes: 200_000,
                hole_bytes: 256,
            },
        )
        .unwrap(),
        u16::from(selected),
        0,
    )
}
fn capacity(f: view::ProposalFragment) -> skein_ui::Limits {
    skein_ui::Limits {
        nodes: f.nodes,
        depth: f.depth,
        text_bytes: f.text_bytes,
        text_per_node: f.text_per_node,
        drawing_bytes: 0,
        styles_per_node: 2,
        patches: 0,
        bindings: f.bindings,
        slots: f.slots,
        markdown_depth: 3,
        scene_marks: 0,
        scene_bytes: 0,
    }
}
fn build(value: &OwnerProposal, w: &Words) -> (Tree, JigBindings) {
    let f = view::required(value, &limits(), w).unwrap();
    let mut b = Builder::new(&capacity(f));
    let mut bindings = JigBindings::new(f.bindings, 256);
    let client = Fixture::new(&limits());
    view::render(&mut b, &client.view(), value, w, &mut bindings);
    let tree = b.finish();
    super::cards_access::check(&tree);
    assert!(tree.len() <= f.nodes);
    let total: u64 = tree
        .nodes()
        .iter()
        .map(|n| {
            assert!(crate::tests::text_bytes(n) <= f.text_per_node);
            u64::from(crate::tests::text_bytes(n))
        })
        .sum();
    assert!(total <= u64::from(f.text_bytes));
    (tree, bindings)
}
fn has(tree: &Tree, bytes: &[u8]) -> bool {
    tree.nodes().iter().any(|n| n.text.iter().any(|r| find(&r.text, bytes).is_some()))
}
fn batch() -> List<ProposedTask> {
    let mut tasks = List::with_capacity(3);
    for n in 0..3 {
        let a = state(b"task effective context", true);
        let mut resources = List::with_capacity(3);
        for _ in 0_u32..3 {
            resources.push(part()).unwrap();
        }
        tasks
            .push(ProposedTask {
                deadline: NewDeadline::Named,
                number: n,
                spec: a.spec,
                executor: OwnerExecutor::Person(PersonAddress::Party(u64::MAX)),
                contract: TaskContractDocument::Approval { state: copy_of(&[255; 32]), part: part() },
                authority: a.authority,
                budget: u64::MAX,
                funder: Funder::Task(u64::MAX),
                dependencies: a.dependencies,
                resources,
                wake: a.wake,
                priority: Some(u32::MAX),
                cut: true,
            })
            .unwrap();
    }
    tasks
}
#[test]
fn owner_proposal_five_whole_actions_keep_effect_coordinates_release_hold_links_and_unscoped_outbox() {
    let w = words(false);
    let task = OwnerTaskLink::Stored { number: u64::MAX, status: None };
    let cases = [
        (OwnerProposalAction::Batch(batch()), b"Proposed batch".as_slice()),
        (
            OwnerProposalAction::Effect {
                part: part(),
                authority: authority(),
                attempt: u64::MAX,
                completion: u32::MAX,
                position: u32::MAX,
                purpose: u64::MAX,
            },
            b"Proposed effect",
        ),
        (
            OwnerProposalAction::ProcedureEffect {
                part: part(),
                authority: authority(),
                step: u64::MAX,
                purpose: u64::MAX,
            },
            b"Proposed effect",
        ),
        (
            OwnerProposalAction::Amend {
                task: task.clone(),
                addressed: None,
                before: Box::new(state(b"before context", true)),
                after: Box::new(state(b"after context", false)),
                reason: doc(b"why amend", true),
                release: None,
            },
            b"Proposed amendment",
        ),
        (
            OwnerProposalAction::Widen { task: task.clone(), before: authority(), after: authority() },
            b"Proposed authority widening",
        ),
        (OwnerProposalAction::Release { task, hold: Some(Box::new(hold())) }, b"Proposed release"),
    ];
    for (action, caption) in cases {
        let v = source(action);
        let original = v.clone();
        let (tree, bindings) = build(&v, &w);
        assert!(has(&tree, caption));
        assert!(has(&tree, b"Proposal number: 18446744073709551615"));
        assert!(has(&tree, b"Proposal project: 4294967295"));
        assert!(has(&tree, b"whole proposal reason"));
        for node in tree.nodes() {
            if let Some(slot) = node.slot
                && let Some(raw) = bindings.owner_part(slot.token)
            {
                assert_eq!(raw.connector, u16::MAX);
                assert_eq!(raw.kind, OwnerPartKind::Outbox);
                assert_eq!(raw, &part());
            }
        }
        match &v.action {
            OwnerProposalAction::Effect { .. } => {
                assert!(has(&tree, b"Effect attempt: 18446744073709551615"));
                assert!(has(&tree, b"Effect completion: 4294967295"));
                assert!(has(&tree, b"Effect position: 4294967295"));
                assert!(has(&tree, b"Effect purpose: 18446744073709551615"));
                assert_eq!(v.funding.len(), 0);
                assert!(!has(&tree, b"Funding needed: 0"));
            }
            OwnerProposalAction::ProcedureEffect { .. } => {
                assert!(has(&tree, b"Procedure step: 18446744073709551615"));
                assert!(has(&tree, b"Effect purpose: 18446744073709551615"));
                assert!(!has(&tree, b"Effect attempt:"));
                assert!(!has(&tree, b"Effect completion:"));
                assert!(!has(&tree, b"Effect position:"));
                assert_eq!(v.funding.len(), 0);
                assert!(!has(&tree, b"Funding needed: 0"));
            }
            OwnerProposalAction::Release { .. } => {
                assert!(has(&tree, b"Hold revision: 18446744073709551615"));
                assert!(has(&tree, b"hold explanation"));
                assert!(has(&tree, b"Prior stage running attempt: 17"));
            }
            _ => {}
        }
        assert_eq!(v, original);
    }
}
#[test]
fn owner_proposal_amendment_contexts_address_and_release_presence_never_join_a_current_task() {
    amendment_contexts(None);
}

#[test]
fn owner_proposal_addressed_amendment_contexts_and_release_presence_never_join_a_current_task() {
    amendment_contexts(Some(AddressChange {
        revision: Revision { value: u64::MAX },
        address: PersonAddress::Party(u64::MAX),
    }));
}

fn amendment_contexts(addressed: Option<AddressChange>) {
    let w = words(false);
    for release in [None, Some(0), Some(u64::MAX)] {
        for after in [None, Some(doc(b"", false)), Some(doc(b"exact effective after", true))] {
            let mut a = state(b"before supplied context", false);
            a.instructions = after.clone();
            let v = source(OwnerProposalAction::Amend {
                task: OwnerTaskLink::Stored { number: 8, status: None },
                addressed,
                before: Box::new(state(b"before supplied context", false)),
                after: Box::new(a),
                reason: doc(b"own action reason", true),
                release,
            });
            let tree = build(&v, &w).0;
            assert!(has(&tree, b"Before proposed change"));
            assert!(has(&tree, b"After proposed change"));
            assert!(has(&tree, b"before supplied context"));
            assert!(has(&tree, b"own action reason"));
            assert_eq!(has(&tree, b"Address change not supplied"), addressed.is_none());
            assert_eq!(has(&tree, b"Proposed release not supplied"), release.is_none());
            if release == Some(0) {
                assert!(has(&tree, b"Proposed release revision: 0"));
            }
            if addressed.is_some() {
                assert!(has(&tree, b"Person-task revision 18446744073709551615"));
            }
            if after.as_ref().is_some_and(|v| v.cut) {
                assert!(has(&tree, b"exact effective after"));
            }
            assert!(!has(&tree, b"whole task instructions"));
            assert!(has(&tree, b"Stored status unavailable"));
        }
    }
}

#[test]
fn owner_proposal_all_funders_independent_optional_amounts_and_holder_variants_preserve_supplied_identity() {
    let w = words(false);
    for funder in [
        Funder::Task(u64::MAX),
        Funder::Pool { project: u32::MAX, person: u64::MAX, period: u64::MAX },
        Funder::Recurring { project: u32::MAX, task: u64::MAX, period: u64::MAX },
        Funder::Period { project: u32::MAX, period: u64::MAX },
    ] {
        for left in [None, Some(0), Some(u64::MAX)] {
            for after in [None, Some(0), Some(u64::MAX)] {
                for as_holder in [false, true] {
                    let mut v = source(OwnerProposalAction::Release {
                        task: OwnerTaskLink::Stored { number: 8, status: None },
                        hold: None,
                    });
                    v.as_holder = as_holder;
                    v.funding.push(OwnerFundingOffer { source: funder, needed: u64::MAX, left, after }).unwrap();
                    let original = v.clone();
                    let tree = build(&v, &w).0;
                    assert!(has(&tree, b"Funding needed: 18446744073709551615"));
                    assert_eq!(has(&tree, b"Funding left not supplied"), left.is_none());
                    assert_eq!(has(&tree, b"Funding after not supplied"), after.is_none());
                    assert_eq!(has(&tree, b"Not proposed as holder"), !as_holder);
                    assert!(has(&tree, b"Release hold not supplied"));
                    if left == Some(0) {
                        assert!(has(&tree, b"Funding left: 0"));
                    }
                    if after == Some(0) {
                        assert!(has(&tree, b"Funding after: 0"));
                    }
                    assert_eq!(v, original);
                }
            }
        }
    }
    for kind in [
        OwnerMessageProposalKind::Batch,
        OwnerMessageProposalKind::Effect,
        OwnerMessageProposalKind::Amend,
        OwnerMessageProposalKind::Widen,
        OwnerMessageProposalKind::Release,
    ] {
        let mut v = source(OwnerProposalAction::Batch(List::with_capacity(3)));
        v.waiting.holder = OwnerHistoryHolder::Policy { project: u32::MAX, kind };
        assert!(has(&build(&v, &w).0, b"Holder policy project: 4294967295"));
    }
}
#[test]
fn owner_proposal_exact_checked_envelopes_keep_selected_language_whole_loss_and_one_under_refusals() {
    let w = words(true);
    let v = source(OwnerProposalAction::Batch(batch()));
    let unchanged = v.clone();
    let (tree, _) = build(&v, &w);
    assert!(has(&tree, b"second 18446744073709551615"));
    assert!(!has(&tree, b"Pending proposal"));
    let f = view::required(&v, &limits(), &w).unwrap();
    assert_eq!(f, view::worst_case(&limits(), &w).unwrap());
    assert_eq!(f.nodes, view::max_nodes(&limits()).unwrap());
    assert!(view::required(&v, &crate::Limits { rows: 2, ..limits() }, &w).is_none());
    assert!(view::required(&v, &crate::Limits { text: 31, ..limits() }, &w).is_none());
    assert_eq!(v, unchanged);
    assert!(view::max_nodes(&crate::Limits { rows: u32::MAX, ..limits() }).is_none());
    assert!(view::worst_case(&crate::Limits { text: u32::MAX, ..limits() }, &w).is_none());
    let id = crate::owner_proposal_words::ProposalPhrase::Attempt.id();
    assert_eq!(w.book().say(1, id, &[Hole::Wall(Wall::EPOCH)]).unwrap_err(), skein_ui::phrases::Invalid::Values { id });
    assert_eq!(w.book().say(1, id, &[Hole::Number(u64::MAX)]).unwrap().text.as_ref(), b"second 18446744073709551615");
    let short = skein_ui::Limits { nodes: tree.len() - 1, ..capacity(f) };
    let _refusal = std::panic::catch_unwind(|| {
        let mut b = Builder::new(&short);
        let mut bindings = JigBindings::new(f.bindings, 256);
        view::render(&mut b, &Fixture::new(&limits()).view(), &v, &w, &mut bindings);
        b.finish()
    })
    .unwrap_err();
}
#[test]
fn owner_proposal_link_presence_and_same_keyed_card_across_contexts_keep_current_facts_and_checked_confirmation() {
    use crate::{Event, action::Act};
    let w = words(false);
    let mut chip = super::cards_objects::task_doc(&[]).chip;
    chip.title = doc(b"supplied linked title", true);
    let links = [
        OwnerTaskLink::Live(chip.clone()),
        OwnerTaskLink::Retained { chip: chip.clone(), at: None },
        OwnerTaskLink::Retained { chip, at: Some(Wall::EPOCH) },
        OwnerTaskLink::Stored { number: 8, status: None },
        OwnerTaskLink::Stored { number: 8, status: Some(OwnerTaskStatus::Done) },
        OwnerTaskLink::Stored { number: 8, status: Some(OwnerTaskStatus::Failed) },
        OwnerTaskLink::Stored { number: 8, status: Some(OwnerTaskStatus::Cancelled) },
    ];
    for link in links {
        let v = source(OwnerProposalAction::Release { task: link.clone(), hold: None });
        let tree = build(&v, &w).0;
        match link {
            OwnerTaskLink::Live(_) => assert!(has(&tree, b"Live task")),
            OwnerTaskLink::Retained { at, .. } => {
                assert!(has(&tree, b"Retained ended task"));
                assert_eq!(has(&tree, b"Ended time unavailable"), at.is_none());
            }
            OwnerTaskLink::Stored { status, .. } => {
                assert!(has(&tree, b"Stored task 8"));
                assert_eq!(has(&tree, b"Stored status unavailable"), status.is_none());
                assert!(!has(&tree, b"supplied linked title"));
            }
        }
    }
    whole_linked_owner_carrier(&w);
    let mut maker_keys = List::with_capacity(2);
    for maker in [Proposer::Task(u64::MAX), Proposer::Party(u64::MAX)] {
        let mut v = source(OwnerProposalAction::Release {
            task: OwnerTaskLink::Stored { number: 8, status: None },
            hold: None,
        });
        v.key.proposer = maker;
        v.funding = List::with_capacity(2);
        v.acts = List::with_capacity(2);
        v.acts.push(Offered { effect: None, act: ActKind::Accept, admission: Admission::Allowed }).unwrap();
        let (mut client, id) =
            super::cards_objects::loaded(ObjectKey::Proposal(v.key), ReadValue::OwnerProposal(Box::new(v.clone())));
        let f = view::worst_case(&limits(), &w).unwrap();
        let mut expected = None;
        for place in [
            crate::cards::Place::Inbox { lineage: List::with_capacity(0) },
            crate::cards::Place::Conversation,
            crate::cards::Place::TaskPage,
            crate::cards::Place::Elsewhere,
        ] {
            let mut b = Builder::new(&skein_ui::Limits { nodes: f.nodes + 100, ..capacity(f) });
            let mut bindings = JigBindings::new(f.bindings, 256);
            assert!(crate::cards::card(&mut b, &client.view(), id, &place, &w, &mut bindings));
            let tree = b.finish();
            super::cards_access::check(&tree);
            assert!(has(&tree, b"Proposed release"));
            assert!(
                tree.nodes()
                    .iter()
                    .any(|node| node.role == skein_ui::Role::Button && node.name.as_deref() == Some(b"Accept"))
            );
            let key = tree.nodes()[0].key.unwrap();
            if let Some(previous) = expected {
                assert_eq!(key, previous);
            } else {
                expected = Some(key);
            }
            assert_eq!(crate::objects::proposal::owner_facts(&client.objects().get(id).unwrap()), Some(&v));
        }
        maker_keys.push(expected.unwrap()).unwrap();
        super::cards::drive(
            &mut client,
            Event::Act { action: Act::Intend { object: id.token(), act: ActKind::Accept } },
        );
        assert_eq!(client.confirmation().unwrap().revision, None);
        let mut b = Builder::new(&skein_ui::Limits { nodes: f.nodes + 100, ..capacity(f) });
        let mut bindings = JigBindings::new(f.bindings, 256);
        crate::confirmation::confirmation(&mut b, &client.view(), &w, &mut bindings);
        let tree = b.finish();
        super::cards_access::check(&tree);
        assert!(has(&tree, b"Proposed release"));
        assert!(has(&tree, b"Release hold not supplied"));
        super::cards::drive(&mut client, Event::Act { action: Act::Dismiss });
        assert!(client.confirmation().is_none());
    }
    assert_ne!(maker_keys.get(0).unwrap(), maker_keys.get(1).unwrap());
}

fn whole_linked_owner_carrier(w: &Words) {
    let mut details = super::owner_chip::details();
    details.hold = Some(hold());
    details.ending = Some(OwnerChipEnding::Failed(doc(b"whole linked failure", true)));
    details.resources = List::with_capacity(3);
    for _ in 0_u32..3 {
        details
            .resources
            .push(OwnerResourceLine {
                part: part(),
                kind: u16::MAX,
                taken: false,
                holder: None,
                writer: Some(OwnerResourceWriter::Effect { entry: u64::MAX }),
                writer_lost: true,
            })
            .unwrap();
    }
    details.resources_cut = true;
    let mut owner_chip = super::cards_objects::task_doc(&[]).chip;
    owner_chip.details = Box::new(ChipPhase::Owner { phase: OwnerChipPhase::Ended, details: Box::new(details) });
    let linked = source(OwnerProposalAction::Release {
        task: OwnerTaskLink::Retained { chip: owner_chip, at: Some(Wall::EPOCH) },
        hold: Some(Box::new(hold())),
    });
    let linked_tree = build(&linked, w).0;
    assert!(has(&linked_tree, b"Retained ended task"));
    assert!(!has(&linked_tree, b"whole linked failure"));
    assert!(!has(&linked_tree, b"Resource kind: 65535"));
    assert!(!has(&linked_tree, b"Writer effect: 18446744073709551615"));
    assert!(!has(&linked_tree, b"Resource list incomplete"));
    let (linked_client, linked_id) = super::cards_objects::loaded(
        ObjectKey::Proposal(linked.key),
        ReadValue::OwnerProposal(Box::new(linked.clone())),
    );
    assert_eq!(crate::objects::proposal::owner_facts(&linked_client.objects().get(linked_id).unwrap()), Some(&linked),);
}

const UTF8_TEXTS: [&[u8]; 1] = [&[b'~'; 32]];
