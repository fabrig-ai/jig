#![expect(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::wildcard_enum_match_arm,
    reason = "ordinary public fragment proofs inspect full supplied sources and tree bindings"
)]
use super::cards::{CLIENT, contains, text, words};
use crate::tests::fixtures::{Fixture, ReadPage, Served, Watched};
use crate::{JigBindings, owner_historical as view};
use skein_ui::Limits;
use skein_ui::{Builder, Role, Tree};

use jig_client::contract::*;
use skein_lib::{List, Wall};
fn task() -> Task {
    let mut task = super::cards_objects::task_doc(&[ActKind::Cancel]);
    task.context = None;
    task
}
fn result() -> Result {
    Result {
        task: task().chip,
        ending: Ending::Done,
        value: Some(ResultValue::Report(DocumentText { bytes: text(b"independent result"), cut: true })),
        at: None,
        details: None,
        acts: List::with_capacity(0),
    }
}
fn envelope(key: ObjectKey, value: OwnerHistoricalValue) -> OwnerHistorical {
    OwnerHistorical { key, value, at: None, by: None }
}
fn person() -> OwnerHistorical {
    let task = task();
    let source = super::owner_history::state();
    let mut path = List::with_capacity(2);
    path.push(Bytes::new(Box::from(b"a/b".as_slice()), 128).unwrap()).unwrap();
    path.push(Bytes::new(Box::from([255, 0]), 128).unwrap()).unwrap();
    let mut holdings = List::with_capacity(2);
    holdings.push(OwnerGoalHolding::Write { connector: u16::MAX, path: path.clone(), kind: u16::MAX }).unwrap();
    holdings.push(OwnerGoalHolding::Slot { connector: 0, path, kind: 0 }).unwrap();
    let goal = OwnerPersonProposalGoal {
        number: u64::MAX,
        project: u32::MAX,
        executor: OwnerExecutor::Agent { charter: u32::MAX },
        spec: Specification {
            asks: false,
            words: text(b"raw goal"),
            parameters: List::with_capacity(0),
            inputs: List::with_capacity(0),
        },
        contract: TaskContractDocument::Report { words: u32::MAX },
        authority: OwnerAuthority {
            grants: List::with_capacity(0),
            delegation: OwnerDelegation { kinds: List::with_capacity(0), tasks: u32::MAX, depth: u32::MAX },
            note_resources: List::with_capacity(0),
            ..source.authority.authority
        },
        numbers: OwnerNumbers { budget: 1, spent: 7, spent_below: 11, reserved: u64::MAX },
        funder: Funder::Task(u64::MAX),
        dependencies: List::with_capacity(0),
        holdings,
        wake: task.wake,
        tracked: Some(u32::MAX),
    };
    envelope(
        ObjectKey::Proposal(ProposalKey { proposer: Proposer::Party(u64::MAX), number: u64::MAX }),
        OwnerHistoricalValue::PersonProposal {
            proposal: Box::new(OwnerPersonProposal {
                number: u64::MAX,
                proposer: u64::MAX,
                project: u32::MAX,
                goal,
                state: OwnerPersonProposalState::Pending { since: Wall::EPOCH },
            }),
            decision: None,
        },
    )
}
fn question(answer: bool, ending: bool) -> OwnerHistorical {
    let q = super::cards_objects::question();
    let answer = if answer {
        Some(OwnerActivityLine {
            task: q.task.task,
            position: u64::MAX,
            at: Wall::from_nanos(17),
            line: OwnerActivity::Words {
                message: 99,
                by: OwnerRequester::Deployment { project: u32::MAX },
                kind: OwnerMessageKind::Answer { question: q.message },
                words: text(UTF8_TEXTS[0]),
            },
        })
    } else {
        None
    };
    envelope(
        ObjectKey::Question { task: q.task.task, message: q.message },
        OwnerHistoricalValue::Question {
            question: q,
            answer,
            ending: if ending { Some(Box::new(result())) } else { None },
        },
    )
}
fn limits(f: view::HistoricalFragment) -> Limits {
    Limits {
        nodes: f.nodes,
        depth: f.depth,
        text_per_node: f.text_per_node,
        text_bytes: f.text_bytes,
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
fn build(value: &OwnerHistorical, words: &crate::words::Words) -> (Tree, JigBindings) {
    let f = view::required(value, &CLIENT, words).unwrap();
    let l = limits(f);
    let domain = Fixture::new(&CLIENT);
    let mut b = Builder::new(&l);
    let mut bindings = JigBindings::new(f.bindings, CLIENT.text);
    view::render(&mut b, &domain.view(), value, words, &mut bindings);
    let tree = b.finish();
    super::cards_access::check(&tree);
    assert!(tree.len() <= f.nodes);
    assert!(tree.nodes().iter().map(crate::tests::text_bytes).sum::<u32>() <= f.text_bytes);
    assert!(
        tree.nodes()
            .iter()
            .all(|node| node.role != Role::Button && node.role != Role::TextField && node.role != Role::TextArea)
    );
    (tree, bindings)
}
#[test]
fn historical_task_renders_whole_task_and_independent_result_without_current_task_controls_or_context_defaults() {
    let row = task();
    assert!(row.context.is_none());
    let value = envelope(
        ObjectKey::Task(8),
        OwnerHistoricalValue::Task { document: Box::new(row), result: Box::new(result()) },
    );
    let (tree, _) = build(&value, words());
    assert!(contains(&tree, b"independent result"));
    assert!(contains(&tree, b"Historical task"));
    assert!(contains(&tree, b"Historical record time not supplied"));
    assert!(contains(&tree, b"Historical record sender not supplied"));
    assert!(tree.nodes().iter().all(|node| node.role != Role::Button));
}
#[test]
fn historical_raw_person_goal_displays_all_four_numbers_and_distinct_unjoined_opaque_holdings() {
    let value = person();
    let (tree, _) = build(&value, words());
    for caption in [
        b"Budget: 1".as_slice(),
        b"Spent: 7",
        b"Spent below: 11",
        b"Reserved: 18446744073709551615",
        b"Write holding",
        b"Slot holding",
        b"a/b",
        b"\\xFF\\x00",
    ] {
        assert!(contains(&tree, caption));
    }
    assert!(!contains(&tree, b"a/b/"));
    assert!(contains(&tree, b"Decision not supplied"));
    let selected = words();
    for (choice, caption) in [
        (OwnerProposalChoice::Accepted, b"Accept decision".as_slice()),
        (OwnerProposalChoice::Rejected, b"Reject decision"),
        (OwnerProposalChoice::Passed, b"Passed"),
        (OwnerProposalChoice::Withdrawn, b"Withdrawn decision"),
        (OwnerProposalChoice::Stale, b"Stale decision"),
    ] {
        let mut decided = value.clone();
        let OwnerHistoricalValue::PersonProposal { decision, .. } = &mut decided.value else {
            panic!("supplied raw person proposal")
        };
        *decision = Some(OwnerProposalDecision {
            project: u32::MAX,
            proposer: Proposer::Party(u64::MAX),
            proposal: u64::MAX,
            kind: OwnerMessageProposalKind::Batch,
            by: u64::MAX,
            at: Wall::EPOCH,
            choice,
            created: None,
        });
        let (tree, _) = build(&decided, selected);
        assert!(contains(&tree, caption));
        assert!(contains(&tree, b"Created task pair not supplied"));
        assert!(tree.nodes().iter().all(|node| node.role != Role::Button));
    }
}
#[test]
fn historical_retained_proposal_uses_bare_whole_fragment_and_keeps_pending_state_without_fake_history_row() {
    let p = super::owner_history::proposal(OwnerRetainedAction::Release { task: u64::MAX });
    let value = envelope(
        ObjectKey::Proposal(ProposalKey { proposer: Proposer::Task(p.proposer), number: p.number }),
        OwnerHistoricalValue::Proposal(p),
    );
    assert!(!value.is_terminal());
    let (tree, _) = build(&value, words());
    assert!(contains(&tree, b"Historical task proposal"));
    assert!(!contains(&tree, b"History revision"));
    assert!(crate::owner_history::proposal_max_nodes(&CLIENT).unwrap() <= view::max_nodes(&CLIENT).unwrap());
}
#[test]
fn historical_escalation_keeps_rejection_revision_sender_and_optional_retained_hold_independently() {
    let value = envelope(
        ObjectKey::Escalation(8),
        OwnerHistoricalValue::Escalation {
            task: Box::new(task()),
            decision: OwnerEscalationDecisionRecord {
                project: 4,
                requester: u64::MAX,
                task: 8,
                revision: u64::MAX,
                by: 17,
                at: Wall::EPOCH,
                decision: OwnerEscalationDecision::Reject { reason: text(UTF8_TEXTS[0]) },
            },
            hold: None,
        },
    );
    let (tree, _) = build(&value, words());
    assert!(contains(&tree, b"Revision: 18446744073709551615"));
    assert!(contains(&tree, b"Decision person: 17"));
    assert!(contains(&tree, b"Retained hold not supplied"));
    assert!(contains(&tree, UTF8_TEXTS[0]));
}
#[test]
fn historical_question_keeps_answer_full_result_and_outer_absence_as_three_independent_sources() {
    let mut value = question(true, true);
    value.at = Some(Wall::from_nanos(23));
    value.by = Some(OwnerRequester::Person(19));
    let (tree, _) = build(&value, words());
    assert!(contains(&tree, b"independent result"));
    assert!(contains(&tree, b"Historical record sender"));
    assert!(!contains(&tree, b"Answer not supplied"));
    assert!(!contains(&tree, b"Ending not supplied"));
    let pending = question(false, false);
    assert!(!pending.is_terminal());
    let (tree, _) = build(&pending, words());
    assert!(contains(&tree, b"Answer not supplied"));
    assert!(contains(&tree, b"Ending not supplied"));
}
#[test]
fn historical_person_task_keeps_whole_address_contract_loss_and_independent_result_without_answer_form() {
    let t = task();
    let document = OwnerPersonTask {
        task: t.chip,
        revision: Revision { value: u64::MAX },
        requester: OwnerRequester::Deployment { project: u32::MAX },
        spec: t.spec,
        address: PersonAddress::Role(u32::MAX),
        taken_by: None,
        since: None,
        stall: None,
        deadline: None,
        contract: TaskContractDocument::Report { words: 32 },
        results: List::with_capacity(0),
        cut: true,
        acts: t.acts,
    };
    let value = envelope(
        ObjectKey::PersonTask(8),
        OwnerHistoricalValue::PersonTask { document: Box::new(document), result: Box::new(result()) },
    );
    let (tree, _) = build(&value, words());
    assert!(contains(&tree, b"independent result"));
    assert!(contains(&tree, b"Historical person task"));
    assert!(tree.nodes().iter().all(|node| node.role != Role::TextField && node.role != Role::TextArea));
}
#[test]
fn historical_whole_receiving_refuses_reserved_paths_and_nested_text_excess_without_clipping_sources() {
    let value = person();
    let original = value.clone();
    assert!(view::required(&value, &CLIENT, words()).is_some());
    assert!(view::required(&value, &crate::Limits { rows: 1, ..CLIENT }, words()).is_none());
    let mut large = value.clone();
    let OwnerHistoricalValue::PersonProposal { proposal, .. } = &mut large.value else { panic!("raw proposal") };
    proposal.goal.spec.words = Text::new(Box::from(UTF8_TEXTS[1]), 129).unwrap();
    assert!(view::required(&large, &CLIENT, words()).is_none());
    assert_eq!(value, original);
    assert!(view::max_nodes(&crate::Limits { rows: u32::MAX, ..CLIENT }).is_none());
}
#[test]
fn historical_passive_card_retains_named_whole_departure_and_does_not_bind_current_question_actions() {
    let value = question(true, false);
    let (d, id) = super::cards_objects::loaded(value.key, ReadValue::Historical(Box::new(value.clone())));
    let f = view::required(&value, &CLIENT, words()).unwrap();
    let mut l = limits(f);
    l.nodes = l.nodes.checked_add(64).unwrap();
    l.text_bytes = l.text_bytes.checked_add(64_u32.checked_mul(l.text_per_node).unwrap()).unwrap();
    let mut b = Builder::new(&l);
    let mut bindings = JigBindings::new(l.bindings, CLIENT.text);
    assert!(crate::cards::card(&mut b, &d.view(), id, &crate::cards::Place::Elsewhere, words(), &mut bindings));
    let tree = b.finish();
    super::cards_access::check(&tree);
    assert!(contains(&tree, b"Historical question"));
    assert!(!contains(&tree, b"Answer not supplied"));
    assert!(tree.nodes().iter().all(|node| node.role != Role::Button));
    let crate::objects::Card::Leaving { how: crate::objects::Left::Historical { facts }, .. } =
        &d.objects().get(id).unwrap().card
    else {
        panic!("whole historical departure")
    };
    assert_eq!(facts.as_ref(), &value);
}
fn step(domain: &mut Fixture, l: &crate::Limits, event: impl crate::tests::fixtures::Input) -> Vec<crate::Request> {
    domain.ui.limits = *l;
    domain.drive(skein_lib::Time::ZERO, event).into_vec()
}
fn presence(mask: u8) {
    use crate::{Event, Request};
    let l = crate::Limits { objects: 4, reads: 8, ..CLIENT };
    let mut domain = presence_domain(&l);
    let proposal_key = ProposalKey { proposer: Proposer::Task(9), number: 7 };
    let mut keys = List::with_capacity(4);
    keys.push(ObjectKey::Task(8)).unwrap();
    if mask & 1 != 0 {
        keys.push(ObjectKey::Result(9)).unwrap();
    }
    if mask & 2 != 0 {
        keys.push(ObjectKey::PersonTask(8)).unwrap();
    }
    if mask & 4 != 0 {
        keys.push(ObjectKey::Proposal(proposal_key)).unwrap();
    }
    let reads = step(&mut domain, &l, Event::Named { sections: List::with_capacity(0), objects: keys });
    let mut task_read = None;
    for request in reads {
        let Request::Client { asked: jig_client::Event::Read { handle: read, read: jig_client::Readable::Jig(what) } } =
            request
        else {
            continue;
        };
        let value = match what {
            Read::Task { task: 8 } => {
                task_read = Some(read);
                continue;
            }
            Read::Result { task: 9 } => {
                let mut value = result();
                value.task.task = 9;
                value.value = Some(ResultValue::Verdict {
                    verdict: 1,
                    words: DocumentText::complete(text(b"whole kept verdict")),
                    fields: List::with_capacity(0),
                    tasks: List::with_capacity(0),
                    proposal: Some(proposal_key),
                });
                ReadValue::Result(Box::new(value))
            }
            Read::PersonTask { task: 8 } => {
                let t = task();
                ReadValue::OwnerPersonTask(Box::new(OwnerPersonTask {
                    task: t.chip,
                    revision: t.spec_revision,
                    requester: t.requester,
                    spec: t.spec,
                    address: PersonAddress::Party(1),
                    taken_by: None,
                    since: None,
                    stall: None,
                    deadline: None,
                    contract: t.result_contract,
                    results: List::with_capacity(0),
                    cut: false,
                    acts: List::with_capacity(0),
                }))
            }
            Read::Proposal { proposal } => {
                let authority = super::cards_objects::authority();
                ReadValue::Proposal(Box::new(Proposal {
                    key: proposal,
                    project: 4,
                    proposer: proposal.proposer,
                    why: text(b"whole kept proposal"),
                    action: ProposalAction::Widen { before: authority.clone(), after: authority.clone() },
                    needs: authority,
                    funding: Budget { total: 1, own: 0, ended: 0, reserved: 0, left: 1, funder: Actor::Task(9) },
                    waiting: Recipient::Party(1),
                    trail: List::with_capacity(0),
                    since: Wall::EPOCH,
                    acts: List::with_capacity(0),
                    ending: None,
                }))
            }
            _ => continue,
        };
        step(&mut domain, &l, ReadPage { read, result: ReadResult::Value(value) });
    }
    presence_finish(&mut domain, &l, task_read.unwrap(), mask);
}
fn presence_finish(domain: &mut Fixture, l: &crate::Limits, task_read: jig_client::Handle, mask: u8) {
    let mut doc = task();
    doc.inputs = List::with_capacity(l.rows);
    for _ in 0..l.rows {
        doc.inputs.push(OwnerTaskLink::Stored { number: 9, status: None }).unwrap();
    }
    let supplied = envelope(
        ObjectKey::Task(8),
        OwnerHistoricalValue::Task { document: Box::new(doc), result: Box::new(result()) },
    );
    step(
        domain,
        l,
        ReadPage { read: task_read, result: ReadResult::Value(ReadValue::Historical(Box::new(supplied.clone()))) },
    );
    let id = domain.objects().find(supplied.key).unwrap();
    let words = words();
    let f = view::required(&supplied, l, words).unwrap();
    let mut cap = limits(f);
    cap.nodes = cap.nodes.checked_add(64).unwrap();
    cap.text_bytes = cap.text_bytes.checked_add(64_u32.checked_mul(cap.text_per_node).unwrap()).unwrap();
    let mut b = Builder::new(&cap);
    let mut bindings = JigBindings::new(cap.bindings, l.text);
    assert!(crate::cards::card(&mut b, &domain.view(), id, &crate::cards::Place::Elsewhere, words, &mut bindings));
    let tree = b.finish();
    super::cards_access::check(&tree);
    assert!(tree.len() <= f.nodes.checked_add(64).unwrap());
    assert_eq!(contains(&tree, b"whole kept verdict"), mask & 1 != 0);
    assert_eq!(contains(&tree, b"whole kept proposal"), mask & 1 != 0 && mask & 4 != 0);
    assert_eq!(
        domain.objects().len(),
        1_u32
            .checked_add(u32::from(mask & 1 != 0))
            .unwrap()
            .checked_add(u32::from(mask & 2 != 0))
            .unwrap()
            .checked_add(u32::from(mask & 4 != 0))
            .unwrap()
    );
    assert!(tree.nodes().iter().all(|node| node.role != Role::Button));
}
#[test]
fn historical_task_absent_result_person_and_proposal_keeps_loading_within_whole_bound() {
    presence(0);
}
#[test]
fn historical_task_result_presence_keeps_repeated_whole_inputs_within_bound() {
    presence(1);
}
#[test]
fn historical_task_person_presence_keeps_whole_supplied_person_within_bound() {
    presence(2);
}
#[test]
fn historical_task_result_and_person_presence_keeps_both_whole_sources_within_bound() {
    presence(3);
}
#[test]
fn historical_task_proposal_presence_keeps_supplied_kept_projection_within_bound() {
    presence(4);
}
#[test]
fn historical_task_result_and_proposal_presence_keeps_original_verdict_key_projection_within_bound() {
    presence(5);
}
#[test]
fn historical_task_person_and_proposal_presence_keeps_independent_sources_within_bound() {
    presence(6);
}
#[test]
fn historical_task_all_three_supplied_presences_keep_repetition_and_whole_sources_within_bound() {
    presence(7);
}
#[test]
fn historical_selected_language_uses_checked_native_captions_and_preserves_opaque_goal_values() {
    use skein_ui::phrases::{PhraseLimits, Phrasebook, Segment};
    let english = crate::phrases::english::language();
    let mut selected = english.clone();
    for phrase in &mut selected.phrases {
        if phrase.id == crate::owner_historical_words::HistoricalPhrase::Historical.id() {
            for form in &mut phrase.forms {
                for segment in &mut form.segments {
                    if let Segment::Text(bytes) = segment {
                        *bytes = Box::from(b"Selected historical source".as_slice());
                    }
                }
            }
        }
    }
    let words = crate::words::Words::new(
        Phrasebook::load(
            crate::phrases::jig::shapes(CLIENT.text),
            Box::new([english, selected]),
            &PhraseLimits {
                languages: 2,
                phrases: crate::owner_words::phrase_count(),
                holes: 4,
                segments: 9000,
                text_bytes: 200_000,
                hole_bytes: CLIENT.text,
            },
        )
        .unwrap(),
        1,
        0,
    );
    let value = person();
    let (tree, _) = build(&value, &words);
    assert_eq!(tree.nodes().first().unwrap().name.as_deref(), Some(b"Selected historical source".as_slice()));
    assert!(contains(&tree, b"\\xFF\\x00"));
    assert!(contains(&tree, b"Reserved: 18446744073709551615"));
    let f = view::required(&value, &CLIENT, &words).unwrap();
    assert!(tree.len() <= f.nodes);
    assert!(tree.nodes().iter().map(crate::tests::text_bytes).sum::<u32>() <= f.text_bytes);
    for phrase in crate::owner_historical_words::ALL {
        assert!(words.book().capacity(phrase.id()).is_some());
    }
}

const UTF8_TEXTS: [&[u8]; 2] = [b"~\0", &[b'~'; 129]];

fn presence_domain(l: &crate::Limits) -> Fixture {
    use crate::{Event, Request};
    let mut domain = Fixture::new(l);
    let first = step(
        &mut domain,
        l,
        Event::Start {
            seed: 8,
            place: crate::address::Place::Jig(crate::address::Address::Inbox { project: None }),
            records: List::with_capacity(0),
        },
    );
    let watch = first
        .iter()
        .find_map(|request| {
            if let Request::Client {
                asked: jig_client::Event::Watch { handle: watch, subject: jig_client::Watchable::Jig(Subject::Me) },
            } = request
            {
                Some(*watch)
            } else {
                None
            }
        })
        .unwrap();
    step(&mut domain, l, Served { served: true });
    step(
        &mut domain,
        l,
        Watched { watch, event: WatchEvent::Delivered(Delivery::Me(Box::new(super::cards::document()))) },
    );
    domain
}
