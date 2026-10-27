use super::*;
use crate::tests::fixtures::{Fixture, ReadPage, Served, Watched};

fn escalation() -> OwnerEscalation {
    let mut task = task_doc(&[ActKind::Stop]);
    task.context = None;
    task.person_revision = Some(Revision { value: 23 });
    task.taken_by = Some(u64::MAX);
    task.result = Some(TaskResult::Ending(Box::new(OwnerChipEnding::Failed(DocumentText::complete(text(
        b"supplied prior task failure",
    ))))));
    task.chip.executor = Executor::Agent { charter: 1 };
    let held = OwnerChipHold {
        placed: None,
        circle: None,
        account: None,
        source: None,
        reason: HoldCategory::Budget,
        revision: u64::MAX,
        before: OwnerPriorPhase::Closing(ClosingStage::Effects),
        escalation: Some(OwnerHoldEscalation::Waiting {
            holder: OwnerEscalationHolder::Person(u64::MAX),
            entry: 17,
            since: Wall::EPOCH,
        }),
        tries: OwnerFailureTries { transient: 1, permanent: 2, run: 3, agent: 4, lost: 5, invalid: 6 },
        hold_wait_since: None,
    };
    let mut details = super::super::owner_chip::details();
    details.project = task.project;
    details.hold = Some(held.clone());
    details.executor = OwnerExecutor::Agent { charter: 1 };
    task.chip.details = Box::new(ChipPhase::Owner {
        phase: OwnerChipPhase::Held { reason: HoldCategory::Budget, revision: u64::MAX },
        details: Box::new(details),
    });
    task.hold = Some(TaskHold::Owner(Box::new(held)));
    OwnerEscalation {
        task: Box::new(task),
        acts: list([
            Offered { effect: None, act: ActKind::Release, admission: Admission::Allowed },
            Offered { effect: None, act: ActKind::Amend, admission: Admission::Allowed },
        ]),
    }
}

fn maximum_card_envelope() {
    let mut row = escalation();
    let whole = vec![b'x'; usize::try_from(CLIENT.text).unwrap()];
    row.task.spec = specification(text(&whole));
    row.task.instructions = Some(DocumentText::complete(text(&whole)));
    row.task.dependencies = list(
        (0..CLIENT.rows)
            .map(|number| OwnerTaskLink::Stored { number: u64::from(number), status: Some(OwnerTaskStatus::Done) }),
    );
    row.task.references = row.task.dependencies.clone();
    row.task.inputs = row.task.dependencies.clone();
    row.task.resources = list((0..CLIENT.rows).map(|number| {
        TaskResource::Part(Part::Resource {
            connector: u16::try_from(number).unwrap(),
            path: list((0..CLIENT.rows).map(|_| Box::<[u8]>::from(whole.clone()))),
        })
    }));
    let original = row.clone();
    let (domain, id) = loaded(ObjectKey::Escalation(8), ReadValue::OwnerEscalation(Box::new(row)));
    let bound = crate::sizes::owner_escalation_nodes(&CLIENT).unwrap().checked_add(1).unwrap();
    let allocation =
        skein_ui::Limits { nodes: bound, depth: 32, text_bytes: bound.checked_mul(1024).unwrap(), ..limits() };
    let mut builder = Builder::new(&allocation);
    let mut bindings = JigBindings::new(bound, CLIENT.text);
    builder.open(Role::Page);
    assert!(cards::card(&mut builder, &domain.view(), id, &cards::Place::Elsewhere, words(), &mut bindings));
    builder.close();
    let tree = builder.finish();
    super::super::cards_access::check(&tree);
    assert!(tree.len() <= bound);
    assert!(contains(&tree, &whole));
    assert_eq!(domain.objects().get(id).unwrap().body, Some(crate::objects::Body::OwnerEscalation(Box::new(original))));
}

#[test]
fn owner_escalation_named_card_keeps_current_holder_prior_stage_and_whole_task_without_other_context() {
    maximum_card_envelope();
    let row = escalation();
    let (domain, id) = loaded(ObjectKey::Escalation(8), ReadValue::OwnerEscalation(Box::new(row.clone())));
    let mut bindings = JigBindings::new(limits().bindings, CLIENT.text);
    let tree = render(&domain, id, words(), &mut bindings);
    assert!(contains(&tree, b"whole task specification"));
    assert!(contains(&tree, b"whole task instructions"));
    assert!(contains(&tree, b"Prior closing effects"));
    assert!(contains(&tree, b"Hold revision: 18446744073709551615"));
    assert!(contains(&tree, b"18446744073709551615"));
    assert!(contains(&tree, b"Budget: 20"));
    assert!(contains(&tree, b"supplied prior task failure"));
    assert!(tree.nodes().iter().any(|node| node.role == Role::Region && node.name.as_deref() == Some(b"Project: 4")));
    assert_eq!(domain.objects().get(id).unwrap().body.as_ref().unwrap().ending(), None);
    let names: Vec<_> = tree.nodes().iter().filter_map(|node| node.name.as_deref()).collect();
    assert!(names.contains(&b"Escalation".as_slice()));
    assert!(tree.nodes().iter().any(|node| node.role == Role::Button && node.name.as_deref() == Some(b"Release")));
    assert!(!tree.nodes().iter().any(|node| node.role == Role::Button && node.name.as_deref() == Some(b"Stop")));
    assert!(tree.len() <= crate::sizes::owner_escalation_nodes(&CLIENT).unwrap() + 1);
    assert_eq!(domain.objects().get(id).unwrap().body, Some(crate::objects::Body::OwnerEscalation(Box::new(row))));
}

#[test]
fn owner_escalation_amendment_inputs_keep_whole_source_baselines_and_selected_deltas_and_loss_guards() {
    for lost in [false, true] {
        let mut row = escalation();
        if lost {
            row.task.cut = true;
            row.task.spec.cut = true;
            row.task.authority.cut = true;
        }
        let original = row.clone();
        let (mut domain, id) = loaded(ObjectKey::Escalation(8), ReadValue::OwnerEscalation(Box::new(row)));
        drive(&mut domain, Event::Act { action: Action::AmendRelease { object: id.token() } });
        let mut bindings = JigBindings::new(limits().bindings, CLIENT.text);
        let tree = dialog(&domain, words(), &mut bindings);
        assert!(contains(&tree, b"whole task specification"));
        assert!(contains(&tree, b"whole task instructions"));
        assert!(contains(&tree, b"Hold revision: 18446744073709551615"));
        let inputs: Vec<_> = tree
            .nodes()
            .iter()
            .filter_map(|node| node.binding.map(|binding| (node.name.as_deref(), binding.token)))
            .collect();
        let budget = inputs.iter().find(|(name, _)| *name == Some(b"New budget")).map(|(_, token)| *token);
        assert_eq!(budget.is_some(), !lost);
        if let Some(token) = budget {
            let act = decode(&mut bindings, token, Act::Input { value: Box::from(*b"9") }).unwrap();
            let Action::Change { change } = act else { panic!("selected amendment") };
            let mut expected = original.task.authority.whole().unwrap();
            expected.spend = 9;
            assert_eq!(change.authority, Some(expected));
            assert_eq!(change.spec, None);
            assert_eq!(change.wake, None);
            assert_eq!(change.dependencies, None);
            drive(&mut domain, Event::Act { action: Action::Change { change } });
            assert_eq!(domain.confirmation().unwrap().revision, Some(original.task.spec_revision));
            assert_eq!(domain.confirmation().unwrap().release_revision, Some(Revision { value: u64::MAX }));
        }
        assert_eq!(
            domain.objects().get(id).unwrap().body,
            Some(crate::objects::Body::OwnerEscalation(Box::new(original)))
        );
    }
}

fn build_closed_offers(row: &OwnerEscalation, allocation: skein_ui::Limits, w: &crate::words::Words) -> Tree {
    let (domain, id) = loaded(ObjectKey::Escalation(8), ReadValue::OwnerEscalation(Box::new(row.clone())));
    let mut builder = Builder::new(&allocation);
    let mut bindings = JigBindings::new(allocation.bindings, CLIENT.text);
    builder.open(Role::Page);
    assert!(cards::card(&mut builder, &domain.view(), id, &cards::Place::Elsewhere, w, &mut bindings));
    builder.close();
    builder.finish()
}

fn closed_offers(refused: bool) -> OwnerEscalation {
    let kinds = [ActKind::Release, ActKind::LeaveHeld, ActKind::PassUp, ActKind::Amend, ActKind::Cancel];
    let mut row = escalation();
    row.acts = List::with_capacity(MAX_OFFERED_ACTS);
    let reason = HoldReason::Tries {
        failures: list([
            AttemptFailure {
                attempt: 0,
                class: FailureClass::HostLost,
                reason: text(b"whole offered failure"),
                at: Wall::EPOCH,
            },
            AttemptFailure {
                attempt: u64::MAX,
                class: FailureClass::Permanent,
                reason: text(b"whole offered failure"),
                at: Wall::from_nanos(u64::MAX),
            },
        ]),
    };
    for _ in 0_u32..5 {
        for act in kinds {
            let admission = if refused {
                Admission::Refused(Refusal::Moved {
                    now: Revision { value: u64::MAX },
                    what: Moved::Hold { reason: Some(CurrentHold::Document(Box::new(reason.clone()))) },
                })
            } else {
                Admission::Allowed
            };
            row.acts.push(Offered { effect: None, act, admission }).unwrap();
        }
    }
    row
}

fn check_exact_storage(row: &OwnerEscalation, allocation: skein_ui::Limits, tree: &Tree, w: &crate::words::Words) {
    let depth = u32::try_from(
        tree.nodes()
            .iter()
            .enumerate()
            .map(|(at, _)| {
                tree.nodes()
                    .iter()
                    .enumerate()
                    .take(at)
                    .filter(|(index, node)| index.checked_add(usize::try_from(node.size).unwrap()).unwrap() > at)
                    .count()
                    .checked_add(1)
                    .unwrap()
            })
            .max()
            .unwrap(),
    )
    .unwrap();
    let per_node = tree.nodes().iter().map(crate::tests::text_bytes).max().unwrap();
    let total = tree.nodes().iter().map(crate::tests::text_bytes).sum::<u32>();
    let exact = skein_ui::Limits { nodes: tree.len(), depth, text_per_node: per_node, text_bytes: total, ..allocation };
    assert_eq!(build_closed_offers(row, exact, w).len(), tree.len());
    for below in [
        skein_ui::Limits { nodes: exact.nodes.checked_sub(1).unwrap(), ..exact },
        skein_ui::Limits { depth: exact.depth.checked_sub(1).unwrap(), ..exact },
        skein_ui::Limits { text_per_node: exact.text_per_node.checked_sub(1).unwrap(), ..exact },
        skein_ui::Limits { text_bytes: exact.text_bytes.checked_sub(1).unwrap(), ..exact },
    ] {
        std::panic::catch_unwind(core::panic::AssertUnwindSafe(|| build_closed_offers(row, below, w)))
            .expect_err("one-under storage refuses the whole escalation tree");
    }
}

fn presence_step(domain: &mut Fixture, l: crate::Limits, event: impl crate::tests::fixtures::Input) -> Vec<Request> {
    domain.ui.limits = l;
    domain.drive(skein_lib::Time::ZERO, event).into_vec()
}

fn presence_result(key: ProposalKey) -> Result {
    let mut result = result();
    result.task.task = 42;
    result.task.details = Box::new(ChipPhase::Waiting { dependencies: list([70, 70]), hold: None });
    result.value = Some(ResultValue::Verdict {
        verdict: u32::MAX,
        words: DocumentText::complete(text(b"repeated whole verdict")),
        fields: list([
            ResultField { name: Box::from(*b"a"), value: Box::from(*b"**whole field**") },
            ResultField { name: Box::from(*b"b"), value: Box::from(*b"**whole field**") },
        ]),
        tasks: list([result.task.clone(), result.task.clone()]),
        proposal: Some(key),
    });
    result.acts = closed_offers(true).acts;
    result
}

fn presence_person(result: &Result, owner: bool) -> ReadValue {
    let mut original = person(PersonContract::Words);
    original.inputs = list([result.clone(), result.clone()]);
    original.acts = closed_offers(true).acts;
    if !owner {
        return ReadValue::PersonTask(Box::new(original));
    }
    ReadValue::OwnerPersonTask(Box::new(OwnerPersonTask {
        task: original.task,
        revision: original.revision,
        requester: OwnerRequester::Person(1),
        spec: original.spec,
        address: PersonAddress::Party(1),
        taken_by: None,
        since: original.since,
        stall: original.stall,
        deadline: original.deadline,
        contract: TaskContractDocument::Report { words: 2 },
        results: list((0..CLIENT.rows).map(|_| OwnerShownResult {
            task: OwnerTaskLink::Retained { chip: result.task.clone(), at: None },
            result: Some(Box::new(result.clone())),
        })),
        cut: true,
        acts: original.acts,
    }))
}

fn presence_domain(l: crate::Limits, documents: &[(ObjectKey, Option<ReadValue>)]) -> Fixture {
    let mut domain = Fixture::new(&l);
    let mut requests = presence_step(
        &mut domain,
        l,
        Event::Start {
            seed: 8,
            place: crate::address::Place::Jig(crate::address::Address::Inbox { project: None }),
            records: List::with_capacity(0),
        },
    );
    requests.extend(presence_step(&mut domain, l, Served { served: true }));
    let frame = requests
        .into_iter()
        .find_map(|request| match request {
            Request::Client {
                asked: jig_client::Event::Watch { handle: watch, subject: jig_client::Watchable::Jig(Subject::Me) },
            } => Some(watch),
            _ => None,
        })
        .unwrap();
    presence_step(
        &mut domain,
        l,
        Watched { watch: frame, event: WatchEvent::Delivered(Delivery::Me(Box::new(document()))) },
    );
    let requests = presence_step(
        &mut domain,
        l,
        Event::Named { sections: List::with_capacity(0), objects: list(documents.iter().map(|(key, _)| *key)) },
    );
    for request in requests {
        if let Request::Client {
            asked: jig_client::Event::Read { handle: read, read: jig_client::Readable::Jig(what) },
        } = request
        {
            let key = match what {
                Read::Escalation { task } => ObjectKey::Escalation(task),
                Read::Result { task } => ObjectKey::Result(task),
                Read::PersonTask { task } => ObjectKey::PersonTask(task),
                Read::Proposal { proposal } => ObjectKey::Proposal(proposal),
                Read::Task { task } => ObjectKey::Task(task),
                _ => panic!("only the exact named fixture objects are read"),
            };
            let value = documents.iter().find(|(named, _)| *named == key).unwrap().1.clone();
            if let Some(value) = value {
                presence_step(&mut domain, l, ReadPage { read, result: ReadResult::Value(value) });
            }
        }
    }
    assert_eq!(domain.objects().len(), l.objects);
    domain
}

fn check_presence_case(present: [bool; 3], loaded: bool, owner: bool) {
    let mut proposal = proposal(ProposalAction::Widen { before: authority(), after: authority() });
    proposal.acts = closed_offers(true).acts;
    let result = presence_result(proposal.key);
    let mut row = closed_offers(true);
    row.task.chip.executor = Executor::Party { party: 1 };
    row.task.inputs = list((0..CLIENT.rows).map(|_| OwnerTaskLink::Retained { chip: result.task.clone(), at: None }));
    row.task.dependencies = row.task.inputs.clone();
    row.task.references = row.task.inputs.clone();
    row.task.result = Some(TaskResult::Document(Box::new(result.clone())));
    let ChipPhase::Owner { details, .. } = row.task.chip.details.as_mut() else { panic!("owned held metadata") };
    details.ending = Some(OwnerChipEnding::Done(result.value.clone().unwrap()));
    let mut documents = vec![(ObjectKey::Escalation(8), Some(ReadValue::OwnerEscalation(Box::new(row.clone()))))];
    for (yes, key, value) in [
        (present[0], ObjectKey::Result(42), ReadValue::Result(Box::new(result))),
        (present[1], ObjectKey::PersonTask(8), presence_person(&presence_result(proposal.key), owner)),
        (present[2], ObjectKey::Proposal(proposal.key), ReadValue::Proposal(Box::new(proposal))),
    ] {
        if yes {
            documents.push((key, loaded.then_some(value)));
        }
    }
    if loaded {
        for (number, dependency) in [(70, 71), (71, 70)] {
            let mut task = task_doc(&[]);
            task.chip.task = number;
            task.chip.title = DocumentText::complete(text(b"reachable stored dependency"));
            task.chip.details =
                Box::new(ChipPhase::Waiting { dependencies: list([dependency, dependency]), hold: None });
            documents.push((ObjectKey::Task(number), Some(ReadValue::Task(Box::new(task)))));
        }
    }
    let l = crate::Limits { objects: u32::try_from(documents.len()).unwrap(), ..CLIENT };
    let domain = presence_domain(l, &documents);
    let id = domain.objects().find(ObjectKey::Escalation(8)).unwrap();
    let bound = crate::sizes::owner_escalation_nodes(&l).unwrap().checked_add(1).unwrap();
    let allocation = skein_ui::Limits {
        nodes: bound,
        depth: 64,
        text_bytes: bound.checked_mul(1024).unwrap(),
        bindings: bound,
        slots: bound,
        ..limits()
    };
    let mut builder = Builder::new(&allocation);
    let mut bindings = JigBindings::new(allocation.bindings, l.text);
    builder.open(Role::Page);
    assert!(cards::card(&mut builder, &domain.view(), id, &cards::Place::Elsewhere, words(), &mut bindings));
    builder.close();
    let tree = builder.finish();
    super::super::cards_access::check(&tree);
    assert!(tree.len() <= bound);
    assert!(
        u32::try_from(tree.nodes().iter().filter(|node| node.binding.is_some()).count()).unwrap()
            <= allocation.bindings
    );
    assert!(contains(&tree, b"whole task specification"));
    assert!(contains(&tree, b"repeated whole verdict"));
    assert_eq!(contains(&tree, b"reachable stored dependency"), loaded);
    if loaded && present[2] {
        let repetitions = tree
            .nodes()
            .iter()
            .filter(|node| node.text.iter().any(|run| skein_lib::bytes::find(&run.text, b"whole reason").is_some()))
            .count();
        let expected = 2_usize
            .checked_add(if present[0] { usize::try_from(CLIENT.rows).unwrap() } else { 0 })
            .unwrap()
            .checked_add(if present[1] { usize::try_from(CLIENT.rows).unwrap() } else { 0 })
            .unwrap();
        assert_eq!(repetitions, expected);
        if present[0] || present[1] {
            assert!(repetitions >= 4);
        }
    }
    assert_eq!(domain.objects().get(id).unwrap().body, Some(crate::objects::Body::OwnerEscalation(Box::new(row))));
}

fn check_presence_envelopes(result: bool, part: u32) {
    assert!(part < 6);
    let mut case = 0_u32;
    for person in [false, true] {
        for proposal in [false, true] {
            for loaded in [false, true] {
                if case / 2 == part {
                    check_presence_case([result, person, proposal], loaded, false);
                }
                case = case.checked_add(1).expect("twelve presence cases");
                if person {
                    if case / 2 == part {
                        check_presence_case([result, person, proposal], loaded, true);
                    }
                    case = case.checked_add(1).expect("twelve presence cases");
                }
            }
        }
    }
    assert_eq!(case, 12);
}

#[test]
fn owner_escalation_without_result_key_keeps_whole_person_inputs_loading_and_repeated_proposals_in_finite_view() {
    check_presence_envelopes(false, 0);
}

#[test]
fn owner_escalation_without_result_key_presence_part_2_keeps_whole_inputs_and_loading() {
    check_presence_envelopes(false, 1);
}

#[test]
fn owner_escalation_without_result_key_presence_part_3_keeps_whole_inputs_and_loading() {
    check_presence_envelopes(false, 2);
}

#[test]
fn owner_escalation_without_result_key_presence_part_4_keeps_whole_inputs_and_loading() {
    check_presence_envelopes(false, 3);
}

#[test]
fn owner_escalation_without_result_key_presence_part_5_keeps_whole_inputs_and_loading() {
    check_presence_envelopes(false, 4);
}

#[test]
fn owner_escalation_without_result_key_presence_part_6_keeps_whole_inputs_and_loading() {
    check_presence_envelopes(false, 5);
}

#[test]
fn owner_escalation_with_repeated_result_key_keeps_whole_person_inputs_loading_and_repeated_proposals_in_finite_view() {
    check_presence_envelopes(true, 0);
}

#[test]
fn owner_escalation_with_repeated_result_key_presence_part_2_keeps_whole_inputs_and_loading() {
    check_presence_envelopes(true, 1);
}

#[test]
fn owner_escalation_with_repeated_result_key_presence_part_3_keeps_whole_inputs_and_loading() {
    check_presence_envelopes(true, 2);
}

#[test]
fn owner_escalation_with_repeated_result_key_presence_part_4_keeps_whole_inputs_and_loading() {
    check_presence_envelopes(true, 3);
}

#[test]
fn owner_escalation_with_repeated_result_key_presence_part_5_keeps_whole_inputs_and_loading() {
    check_presence_envelopes(true, 4);
}

#[test]
fn owner_escalation_with_repeated_result_key_presence_part_6_keeps_whole_inputs_and_loading() {
    check_presence_envelopes(true, 5);
}

#[test]
fn owner_escalation_closed_outer_controls_and_whole_refusals_fit_finite_view_and_refuse_one_under_storage() {
    let w = words();
    for refused in [false, true] {
        let row = closed_offers(refused);
        let original = row.clone();
        assert_eq!(row.acts.capacity(), MAX_OFFERED_ACTS);
        assert_eq!(row.acts.len(), MAX_OFFERED_ACTS);
        assert_eq!(row.task.context, None);
        let (domain, id) = loaded(ObjectKey::Escalation(8), ReadValue::OwnerEscalation(Box::new(row.clone())));
        let mut allocation = crate::limits::of_words(&CLIENT, w).unwrap();
        allocation.nodes = crate::sizes::owner_escalation_nodes(&CLIENT).unwrap() + 1;
        allocation.text_bytes = allocation.nodes * allocation.text_per_node;
        let mut builder = Builder::new(&allocation);
        let mut bindings = JigBindings::new(allocation.bindings, CLIENT.text);
        builder.open(Role::Page);
        assert!(cards::card(&mut builder, &domain.view(), id, &cards::Place::Elsewhere, w, &mut bindings));
        builder.close();
        let tree = builder.finish();
        super::super::cards_access::check(&tree);
        assert!(tree.len() <= allocation.nodes);
        let decoded: Vec<_> = tree
            .nodes()
            .iter()
            .filter_map(|node| {
                if node.role != Role::Button {
                    return None;
                }
                let binding = node.binding?;
                match decode(&mut bindings, binding.token, Act::Press) {
                    Some(Action::Intend { object, act }) => {
                        assert_eq!(object, id.token());
                        Some(act)
                    }
                    _ => None,
                }
            })
            .collect();
        if refused {
            assert!(decoded.is_empty());
            assert!(contains(&tree, b"whole offered failure"));
            assert!(
                tree.nodes()
                    .iter()
                    .filter(|node| node.text.iter().any(|run| skein_lib::bytes::find(
                        &run.text,
                        b"whole offered failure"
                    )
                    .is_some()))
                    .count()
                    >= usize::try_from(MAX_OFFERED_ACTS).unwrap()
            );
        } else {
            assert_eq!(decoded, row.acts.iter().map(|offer| offer.act).collect::<Vec<_>>());
        }
        assert!(contains(&tree, b"whole task specification"));
        assert!(contains(&tree, b"supplied prior task failure"));
        assert_eq!(domain.objects().get(id).unwrap().body.as_ref().unwrap().ending(), None);
        check_exact_storage(&row, allocation, &tree, w);
        assert_eq!(row, original);
        assert_eq!(
            domain.objects().get(id).unwrap().body,
            Some(crate::objects::Body::OwnerEscalation(Box::new(original)))
        );
    }
    let mut oversized = escalation();
    oversized.acts = List::with_capacity(MAX_OFFERED_ACTS + 1);
    let original = oversized.clone();
    let (domain, id) = loaded(ObjectKey::Escalation(8), ReadValue::OwnerEscalation(Box::new(oversized)));
    let allocation = crate::limits::of_words(&CLIENT, w).unwrap();
    let mut builder = Builder::new(&allocation);
    let mut bindings = JigBindings::new(allocation.bindings, CLIENT.text);
    builder.open(Role::Page);
    assert!(!cards::card(&mut builder, &domain.view(), id, &cards::Place::Elsewhere, w, &mut bindings));
    builder.close();
    assert_eq!(builder.finish().len(), 1);
    assert_eq!(domain.objects().get(id).unwrap().body, Some(crate::objects::Body::OwnerEscalation(Box::new(original))));
    assert_eq!(crate::sizes::owner_escalation_nodes(&crate::Limits { rows: u32::MAX, ..CLIENT }), None);
}
