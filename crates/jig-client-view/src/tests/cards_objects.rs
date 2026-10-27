#![expect(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::disallowed_macros,
    clippy::wildcard_enum_match_arm,
    reason = "ordinary tests inspect trees and script public domain requests"
)]
use super::cards::{CLIENT, contains, document, drive, limits, start, text, words};
use crate::tests::fixtures::refusal_answer;
use crate::tests::fixtures::{AnsweredByEngine, Fixture, ReadPage, Watched};
use crate::{JigBindings, cards, decode};
use skein_ui::Act;
use skein_ui::{Builder, Role, State, Tree};

use crate::{Event, Request, action::Act as Action, fields::Purpose, objects::Object};
use jig_client::contract::*;
use skein_lib::{Duration, Id, List, Wall};

fn list<T>(rows: impl IntoIterator<Item = T>) -> List<T> {
    let rows: Vec<T> = rows.into_iter().collect();
    let mut out = List::with_capacity(u32::try_from(rows.len()).unwrap());
    for row in rows {
        assert!(out.push(row).is_ok());
    }
    out
}
fn chip() -> Chip {
    Chip {
        task: 8,
        title: DocumentText::complete(text(b"Review")),
        executor: Executor::Party { party: 1 },
        phase: Phase::Held,
        details: Box::new(ChipPhase::Held {
            reason: HoldReason::Budget { spent: 10, limit: 10 },
            since: Wall::EPOCH,
            revision: Revision { value: 4 },
            was: None,
        }),
    }
}
pub(super) fn owner_authority() -> OwnerAuthorityDocument {
    OwnerAuthorityDocument {
        authority: OwnerAuthority {
            models: None,
            tools: 0,
            grants: List::with_capacity(0),
            delegation: OwnerDelegation { kinds: List::with_capacity(0), tasks: 3, depth: 2 },
            spend: 10,
            deadline: None,
            notes: 0,
            note_resources: List::with_capacity(0),
        },
        cut: false,
    }
}
pub(super) fn authority() -> Authority {
    Authority {
        models: None,
        budget: 10,
        grants: list([]),
        task_kinds: list([Executor::Party { party: 1 }]),
        depth: 2,
        tasks: 3,
        deadline: Some(Wall::EPOCH),
        tools: list([2]),
        note_scopes: list([NoteScope::Deployment]),
    }
}
fn offered(act: ActKind) -> List<Offered> {
    list([
        Offered { effect: None, act, admission: Admission::Allowed },
        Offered {
            effect: None,
            act: ActKind::PassUp,
            admission: Admission::Refused(Refusal::Authority {
                lacking: Lacking::Budget { needed: 40, left: 12 },
                proposable: None,
            }),
        },
    ])
}
fn proposal(action: ProposalAction) -> Proposal {
    Proposal {
        key: ProposalKey { proposer: Proposer::Task(8), number: 1 },
        project: 4,
        proposer: Proposer::Task(8),
        why: text(b"**whole reason**"),
        action,
        needs: authority(),
        funding: Budget { total: 20, own: 0, ended: 0, reserved: 0, left: 20, funder: Actor::Party(1) },
        waiting: Recipient::Role { project: 4, role: 2 },
        trail: list([Passed { by: Actor::Party(3), at: Wall::EPOCH, reason: text(b"whole trail"), lacking: None }]),
        since: Wall::EPOCH,
        acts: offered(ActKind::Accept),
        ending: None,
    }
}
pub(super) fn question() -> Question {
    Question {
        task: chip(),
        from: None,
        message: 1,
        words: text(b"whole question"),
        since: Wall::EPOCH,
        acts: offered(ActKind::Answer),
        ending: None,
    }
}
fn person(contract: PersonContract) -> PersonTask {
    PersonTask {
        task: chip(),
        revision: Revision { value: 4 },
        requester: Actor::Task(1),
        spec: specification(text(b"whole specification")),
        contract,
        inputs: list([result()]),
        addressed: Recipient::Party(1),
        taken: Some(TakenBy { number: document().party.number, name: Some(document().party.name.bytes) }),
        since: Some(Wall::EPOCH),
        stall: Some(Duration::from_secs(240)),
        deadline: None,
        acts: offered(ActKind::Answer),
        ending: None,
    }
}
fn result() -> Result {
    Result {
        task: chip(),
        ending: Ending::Failed { reason: Some(text(b"whole failure")) },
        value: Some(ResultValue::Report(DocumentText::complete(text(b"whole report")))),
        at: Some(Wall::EPOCH),
        acts: offered(ActKind::Seen),
        details: None,
    }
}
fn loaded_with_watch(key: ObjectKey, value: ReadValue) -> (Fixture, Id<Object>, jig_client::Handle) {
    let (mut d, frame) = start();
    let out = drive(&mut d, Watched { watch: frame, event: WatchEvent::Delivered(Delivery::Me(Box::new(document()))) });
    let inbox =
        out.iter()
            .find_map(|r| match r {
                Request::Client {
                    asked:
                        jig_client::Event::Watch {
                            handle: watch,
                            subject: jig_client::Watchable::Jig(Subject::Inbox { .. }),
                        },
                } => Some(*watch),
                _ => None,
            })
            .unwrap();
    let out = drive(
        &mut d,
        Watched {
            watch: inbox,
            event: WatchEvent::Delivered(Delivery::Inbox(Box::new(Inbox {
                entries: list([InboxEntry {
                    reply: None,
                    lineage: list([]),
                    goal: None,
                    recipient: None,
                    stall: None,
                    object: key,
                    project: 4,
                    group: InboxGroup::Questions,
                    position: 1,
                    at: Wall::EPOCH,
                }]),
                needs: document().needs,
                updates: Count { value: 0, limited: false },
                results_seen: 0,
                replies_seen: list([]),
                more: false,
            }))),
        },
    );
    let read = out
        .iter()
        .find_map(|r| match r {
            Request::Client { asked: jig_client::Event::Read { handle: read, .. } } => Some(*read),
            _ => None,
        })
        .unwrap();
    drive(&mut d, ReadPage { read, result: ReadResult::Value(value) });
    let id = d.objects().find(key).unwrap();
    (d, id, inbox)
}
pub(super) fn loaded(key: ObjectKey, value: ReadValue) -> (Fixture, Id<Object>) {
    let (d, id, _) = loaded_with_watch(key, value);
    (d, id)
}
pub(super) fn render(d: &Fixture, id: Id<Object>, words: &crate::words::Words, bindings: &mut JigBindings) -> Tree {
    bindings.begin();
    let mut b = Builder::new(&limits());
    b.open(Role::Page);
    cards::card(&mut b, &d.view(), id, &cards::Place::Elsewhere, words, bindings);
    b.close();
    let tree = b.finish();
    super::cards_access::check(&tree);
    assert!(tree.len() <= crate::card_nodes(&CLIENT).unwrap().checked_add(1).unwrap());
    let facts = d.objects().get(id).unwrap();
    match facts.card {
        crate::objects::Card::Deciding { .. } => {
            assert!(tree.nodes().iter().any(|node| node.styles.as_slice().contains(&crate::JigStyle::Card.token())
                && node.states.as_slice().contains(&State::Busy)));
        }
        crate::objects::Card::Open
        | crate::objects::Card::Refused { .. }
        | crate::objects::Card::Unknown { .. }
        | crate::objects::Card::NotSent { .. }
        | crate::objects::Card::Leaving { .. } => {}
    }
    tree
}
fn render_with_confirmation(
    d: &Fixture,
    id: Id<Object>,
    words: &crate::words::Words,
    bindings: &mut JigBindings,
) -> Tree {
    bindings.begin();
    let bound = crate::card_nodes(&CLIENT)
        .unwrap()
        .checked_add(crate::confirmation_nodes(&CLIENT).unwrap())
        .unwrap()
        .checked_add(1)
        .unwrap();
    let mut cap = limits();
    cap.nodes = bound;
    cap.text_bytes = bound.checked_mul(cap.text_per_node).unwrap();
    let mut b = Builder::new(&cap);
    b.open(Role::Page);
    cards::card(&mut b, &d.view(), id, &cards::Place::Elsewhere, words, bindings);
    crate::confirmation::confirmation(&mut b, &d.view(), words, bindings);
    b.close();
    let tree = b.finish();
    super::cards_access::check(&tree);
    assert!(tree.len() <= bound);
    tree
}
fn press(tree: &Tree, label: &[u8], bindings: &mut JigBindings) -> Action {
    let token = tree
        .nodes()
        .iter()
        .find(|n| n.role == Role::Button && n.name.as_deref() == Some(label))
        .unwrap()
        .binding
        .unwrap()
        .token;
    decode(bindings, token, Act::Press).unwrap()
}

fn whole_object_cards_keep_facts_while_confirmation_request_refusal_and_leaving_change(words: &crate::words::Words) {
    let p = proposal(ProposalAction::Widen { before: authority(), after: authority() });
    for (key, value, label, whole) in [
        (
            ObjectKey::Proposal(p.key),
            ReadValue::Proposal(Box::new(p)),
            b"Accept".as_slice(),
            b"whole reason".as_slice(),
        ),
        (
            ObjectKey::Question { task: 8, message: 1 },
            ReadValue::Question(Box::new(question())),
            b"Answer",
            b"whole question",
        ),
        (
            ObjectKey::PersonTask(8),
            ReadValue::PersonTask(Box::new(person(PersonContract::Words))),
            b"Answer",
            b"whole specification",
        ),
    ] {
        let (mut d, id) = loaded(key, value);
        let mut bindings = JigBindings::new(64, CLIENT.text);
        let tree = render_with_confirmation(&d, id, words, &mut bindings);
        assert!(contains(&tree, whole));
        assert!(contains(&tree, b"Your pool has 12 left; this needs 40"));
        assert!(!tree.nodes().iter().any(|n| n.role == Role::Button && n.name.as_deref() == Some(b"Pass up")));
        let action = press(&tree, label, &mut bindings);
        drive(&mut d, Event::Act { action });
        let tree = render_with_confirmation(&d, id, words, &mut bindings);
        assert!(contains(&tree, whole));
        assert!(contains(&tree, b"review the confirmation"));
        if label == b"Answer" {
            let field = tree
                .nodes()
                .iter()
                .find(|n| n.role == Role::TextArea && n.name.as_deref() == Some(b"Your answer"))
                .unwrap()
                .binding
                .unwrap()
                .token;
            let edit =
                decode(&mut bindings, field, Act::Input { value: Box::from(b"complete answer".as_slice()) }).unwrap();
            drive(&mut d, Event::Act { action: edit });
            assert_eq!(d.fields().get(Purpose::Answer).unwrap().text.bytes(), b"complete answer");
        }
        let out = drive(&mut d, Event::Act { action: Action::Confirm });
        let request = out
            .iter()
            .find_map(|r| match r {
                Request::Client { asked: jig_client::Event::Send { key: request, .. } } => Some(*request),
                _ => None,
            })
            .unwrap();
        let tree = render_with_confirmation(&d, id, words, &mut bindings);
        assert!(contains(&tree, b"Being decided"));
        assert!(contains(&tree, whole));
        drive(
            &mut d,
            AnsweredByEngine {
                request,
                answer: refusal_answer(Refusal::Moved { now: Revision { value: 19 }, what: Moved::PersonTask }),
            },
        );
        let tree = render_with_confirmation(&d, id, words, &mut bindings);
        assert!(contains(&tree, b"19"));
        assert!(contains(&tree, whole));
        let action = press(&tree, label, &mut bindings);
        drive(&mut d, Event::Act { action });
        if label == b"Answer" {
            drive(&mut d, Event::Act { action: Action::Edit { field: Purpose::Answer, text: text(b"answer") } });
        }
        let out = drive(&mut d, Event::Act { action: Action::Confirm });
        let request = out
            .iter()
            .find_map(|r| match r {
                Request::Client { asked: jig_client::Event::Send { key: request, .. } } => Some(*request),
                _ => None,
            })
            .unwrap();
        drive(
            &mut d,
            AnsweredByEngine {
                request,
                answer: Answer::Done(Outcome::DecidedFirst {
                    by: super::cards::party(),
                    how: Decision::Accepted,
                    at: Wall::EPOCH,
                }),
            },
        );
        assert!(contains(&render(&d, id, words, &mut bindings), whole));
    }
}

fn person_contracts_and_result_values_keep_choices_slots_and_full_payloads(words: &crate::words::Words) {
    let part = Part::Result { connector: 2, task: 8 };
    for contract in [
        PersonContract::Choice {
            options: list([OptionLine {
                number: 3,
                words: text(b"Third choice"),
                words_max: 64,
                followups: 0,
                fields: list([]),
            }]),
        },
        PersonContract::Approval { state: text(b"Named state"), part: part.clone() },
        PersonContract::Words,
    ] {
        let is_choice = match &contract {
            PersonContract::Choice { .. } => true,
            PersonContract::Approval { .. } | PersonContract::Words => false,
        };
        let (mut d, id) = loaded(ObjectKey::PersonTask(8), ReadValue::PersonTask(Box::new(person(contract))));
        let mut bindings = JigBindings::new(64, CLIENT.text);
        let before = render(&d, id, words, &mut bindings);
        assert!(!before.nodes().iter().any(|node| node.role == Role::Choice));
        if is_choice {
            let option = before
                .nodes()
                .iter()
                .find(|node| node.role == Role::Item && node.name.as_deref() == Some(b"Third choice"))
                .unwrap();
            assert!(option.binding.is_none());
        }
        let action = press(&before, b"Answer", &mut bindings);
        drive(&mut d, Event::Act { action });
        let tree = render_with_confirmation(&d, id, words, &mut bindings);
        assert!(contains(&tree, b"whole report"));
        assert!(contains(&tree, b"Becomes news for its requester after 4 min"));
        for node in tree.nodes() {
            if let Some(slot) = node.slot {
                assert_eq!(bindings.part(slot.token), Some(&part));
                assert_eq!(decode(&mut bindings, slot.token, Act::Press), None);
            }
            if node.role == Role::Choice {
                let act = decode(&mut bindings, node.binding.unwrap().token, Act::Press).unwrap();
                assert_eq!(act, Action::Choose { option: 3 });
                drive(&mut d, Event::Act { action: act });
            }
        }
        assert!(
            tree.nodes().iter().any(|node| node.role == Role::Button && node.name.as_deref() == Some(b"Cannot do it"))
        );
    }
    let (mut result_domain, id, watch) = loaded_with_watch(ObjectKey::Result(8), ReadValue::Result(Box::new(result())));
    let mut bindings = JigBindings::new(64, CLIENT.text);
    let tree = render(&result_domain, id, words, &mut bindings);
    assert!(contains(&tree, b"whole failure"));
    drive(&mut result_domain, Event::Act { action: Action::Intend { object: id.token(), act: ActKind::PassUp } });
    let tree = render(&result_domain, id, words, &mut bindings);
    assert!(contains(&tree, b"Your pool has 12 left; this needs 40"));
    assert!(contains(&tree, b"whole report"));
    let out = drive(
        &mut result_domain,
        Watched {
            watch,
            event: WatchEvent::Delivered(Delivery::Inbox(Box::new(Inbox {
                entries: list([]),
                needs: document().needs,
                updates: Count { value: 0, limited: false },
                results_seen: 0,
                replies_seen: list([]),
                more: false,
            }))),
        },
    );
    let read = out
        .iter()
        .find_map(|r| match r {
            Request::Client { asked: jig_client::Event::Read { handle: read, .. } } => Some(*read),
            _ => None,
        })
        .unwrap();
    drive(&mut result_domain, ReadPage { read, result: ReadResult::Value(ReadValue::Result(Box::new(result()))) });
    let tree = render(&result_domain, id, words, &mut bindings);
    assert!(tree.nodes().iter().any(|n| n.role == Role::Status && n.name.as_deref() == Some(b"Leaving")));
    assert!(contains(&tree, b"whole failure"));
    assert!(contains(&tree, b"whole report"));
    result_value_payloads(words, &part);
}

fn result_value_payloads(words: &crate::words::Words, part: &Part) {
    let (d, _) = start();
    for (value, whole) in [
        (ResultValue::Report(DocumentText::complete(text(b"full report"))), b"full report".as_slice()),
        (ResultValue::Words(DocumentText::complete(text(b"full words"))), b"full words"),
        (
            ResultValue::Verdict {
                verdict: 2,
                words: DocumentText::complete(text(b"explanation")),
                fields: list([ResultField {
                    name: b"field".as_slice().into(),
                    value: b"full field".as_slice().into(),
                }]),
                tasks: list([]),
                proposal: None,
            },
            b"full field",
        ),
        (
            ResultValue::Connector { summary: DocumentText::complete(text(b"full connector")), part: part.clone() },
            b"full connector",
        ),
        (
            ResultValue::Choice {
                option: 3,
                by: super::cards::party(),
                reason: DocumentText::complete(text(b"full reason")),
            },
            b"full reason",
        ),
        (
            ResultValue::Approved {
                state: b"full state".as_slice().into(),
                words: Some(DocumentText::complete(text(b"approved words"))),
            },
            b"full state",
        ),
        (ResultValue::Failure(DocumentText::complete(text(b"full failure"))), b"full failure"),
    ] {
        let mut b = Builder::new(&limits());
        b.open(Role::Page);
        let mut bindings = JigBindings::new(64, CLIENT.text);
        cards::result::value_nodes(&mut b, &d.view(), &value, words, &mut bindings);
        b.close();
        assert!(contains(&b.finish(), whole));
    }
}

fn proposal_kinds_chip_phases_and_live_replies_keep_facts_and_slots(words: &crate::words::Words) {
    let part = Part::Effect { connector: 2, proposal: ProposalKey { proposer: Proposer::Task(8), number: 9 } };
    for (action, whole) in [
        (
            ProposalAction::Batch {
                own: true,
                tasks: list([ProposedTask {
                    number: 3,
                    spec: specification(text(b"full batch spec")),
                    executor: OwnerExecutor::Person(PersonAddress::Party(1)),
                    budget: 40,
                    authority: OwnerAuthorityDocument {
                        authority: OwnerAuthority {
                            deadline: Some(Wall::EPOCH),
                            ..super::work_pages::recurring_task(3, b"full batch spec").authority.authority
                        },
                        cut: false,
                    },
                    dependencies: list([2]),
                    ..super::work_pages::recurring_task(3, b"full batch spec")
                }]),
            },
            b"full batch spec".as_slice(),
        ),
        (ProposalAction::Effect { part: part.clone() }, b"Effect"),
        (ProposalAction::Widen { before: authority(), after: authority() }, b"After"),
        (
            ProposalAction::Amend {
                task: 8,
                before: text(b"full before"),
                change: Amendment {
                    llm: None,
                    addressed: None,
                    spec: Some(specification(text(b"full after")).whole().unwrap()),
                    instructions: None,
                    wake: None,
                    dependencies: None,
                    authority: None,
                    tracked: None,
                },
            },
            b"full after",
        ),
    ] {
        let mut b = Builder::new(&limits());
        b.open(Role::Page);
        let mut bindings = JigBindings::new(64, CLIENT.text);
        cards::proposal::action(&mut b, &action, words, &mut bindings, CLIENT.rows);
        b.close();
        let tree = b.finish();
        super::cards_access::check(&tree);
        assert!(contains(&tree, whole));
        for node in tree.nodes() {
            if let Some(slot) = node.slot {
                assert_eq!(bindings.part(slot.token), Some(&part));
            }
        }
    }
    all_chip_phases(words);
    live_reply_and_dependency_chips(words);
}

fn all_chip_phases(words: &crate::words::Words) {
    let (d, _) = start();
    let part = Part::Panel { connector: 2, task: 8 };
    for (details, expected) in [
        (
            ChipPhase::Waiting { dependencies: list([99]), hold: Some(part.clone()) },
            b"Loading task 99 phase facts".as_slice(),
        ),
        (
            ChipPhase::Idle {
                wake: WakePolicy { words: true, results: true, topics: list([3]), subscriptions: list([part.clone()]) },
            },
            b"Subscribed topic 3",
        ),
        (ChipPhase::Starting, b"Starting"),
        (
            ChipPhase::Running { attempt: 2, since: Wall::EPOCH, tool: Some(text(b"search tool")) },
            b"Using tool search tool",
        ),
        (ChipPhase::Running { attempt: 2, since: Wall::EPOCH, tool: None }, b"Writing"),
        (ChipPhase::WaitingForWords { attempt: 3, since: Wall::EPOCH }, b"Waiting for your words"),
        (
            ChipPhase::BackingOff {
                at: Wall::EPOCH,
                attempt: 2,
                tries: 3,
                class: FailureClass::HostLost,
                reason: text(b"actual retry reason"),
            },
            b"actual retry reason",
        ),
        (ChipPhase::Procedure { panel: part.clone() }, b"Task 8: Review"),
        (
            ChipPhase::Person {
                addressed: Recipient::Role { project: 4, role: 2 },
                taken: Some(TakenBy { number: document().party.number, name: Some(document().party.name.bytes) }),
                since: Some(Wall::EPOCH),
            },
            b"Taken by Pat",
        ),
        (
            ChipPhase::Closing(Closing { delegates: 2, connectors: Some(list([1])), stage: None, ending: None }),
            b"Closing: 2 delegates",
        ),
        (
            ChipPhase::Held {
                reason: HoldReason::Stopped { by: super::cards::party(), at: Wall::EPOCH },
                since: Wall::EPOCH,
                revision: Revision { value: 7 },
                was: None,
            },
            b"Held: stopped by Pat",
        ),
        (
            ChipPhase::Ended {
                how: Ending::Cancelled { by: Actor::Party(1), reason: Some(text(b"actual cancellation")), at: None },
                at: Wall::EPOCH,
            },
            b"actual cancellation",
        ),
    ] {
        let mut value = chip();
        value.phase = match &details {
            ChipPhase::Owner { phase, .. } => phase.compact(),
            ChipPhase::Waiting { .. } => Phase::Waiting,
            ChipPhase::Idle { .. } => Phase::Idle,
            ChipPhase::Starting => Phase::Due,
            ChipPhase::Running { .. } | ChipPhase::WaitingForWords { .. } => Phase::Running,
            ChipPhase::BackingOff { .. } => Phase::BackingOff,
            ChipPhase::Active(_) | ChipPhase::Procedure { .. } | ChipPhase::Person { .. } => Phase::Active,
            ChipPhase::Closing(_) => Phase::Closing,
            ChipPhase::Held { .. } => Phase::Held,
            ChipPhase::Ended { how, .. } => match how {
                Ending::Done | Ending::Decided { .. } | Ending::Deleted => Phase::Done,
                Ending::Task(_) => Phase::Ended,
                Ending::Failed { .. } => Phase::Failed,
                Ending::Cancelled { .. } | Ending::Withdrawn { .. } => Phase::Cancelled,
            },
        };
        *value.details = details;
        let mut b = Builder::new(&limits());
        b.open(Role::Page);
        let mut bindings = JigBindings::new(64, CLIENT.text);
        cards::chip::chip(&mut b, &d.view(), &value, words, &mut bindings);
        b.close();
        let tree = b.finish();
        super::cards_access::check(&tree);
        assert!(contains(&tree, expected), "phase text {expected:?}");
        assert!(contains(&tree, b"Task 8: Review"));
        for node in tree.nodes() {
            if let Some(slot) = node.slot {
                let expected =
                    if value.phase == Phase::Closing { Part::Connector { connector: 1 } } else { part.clone() };
                assert_eq!(bindings.part(slot.token), Some(&expected));
            }
        }
        if contains(&tree, b"Attempt") {
            assert!(contains(&tree, b"For 0 min"));
        }
    }
}

fn dependency_reply(dependency: Chip) -> TaskLine {
    TaskLine {
        chip: dependency,
        project: 4,
        requester: OwnerRequester::Person(1),
        tracked: None,
        label: 1,
        flags: TaskFlags {
            ended_by: EndedBy::Result,
            conversation: true,
            listed: false,
            retention: Retention::ForgottenOnceRead,
        },
        changed: Wall::EPOCH,
        last_activity: Wall::EPOCH,
        ended_at: None,
        budget: OwnerBudget {
            numbers: OwnerNumbers { budget: 0, spent: 0, spent_below: 0, reserved: 0 },
            left: 0,
            funder: Funder::Pool { project: 4, person: 1, period: 3 },
            deadline: None,
        },
        waits_on: WaitsOn::Nothing,
        phases: OwnerPhaseCounts {
            waiting: 0,
            idle: 0,
            starting: 0,
            running: 0,
            backing_off: 0,
            closing: 0,
            held: 0,
            done: 0,
            failed: 0,
            cancelled: 0,
        },
        held_below: 0,
        conversation: Some(ConversationRead { seen: 5, unread: Count { value: 2, limited: false } }),
    }
}

fn live_reply_and_dependency_chips(words: &crate::words::Words) {
    let (mut d, frame) = start();
    let out = drive(&mut d, Watched { watch: frame, event: WatchEvent::Delivered(Delivery::Me(Box::new(document()))) });
    let inbox =
        out.iter()
            .find_map(|r| match r {
                Request::Client {
                    asked:
                        jig_client::Event::Watch {
                            handle: watch,
                            subject: jig_client::Watchable::Jig(Subject::Inbox { .. }),
                        },
                } => Some(*watch),
                _ => None,
            })
            .unwrap();
    let mut dependency = chip();
    dependency.task = 9;
    dependency.title = DocumentText::complete(text(b"Live dependency"));
    dependency.phase = Phase::Running;
    *dependency.details = ChipPhase::Running { attempt: 2, since: Wall::EPOCH, tool: Some(text(b"actual tool")) };
    let conversation = dependency_reply(dependency);
    drive(
        &mut d,
        Watched {
            watch: inbox,
            event: WatchEvent::Delivered(Delivery::Inbox(Box::new(Inbox {
                entries: list([InboxEntry {
                    reply: Some(Box::new(conversation)),
                    lineage: list([]),
                    goal: None,
                    recipient: None,
                    stall: None,
                    object: ObjectKey::Task(9),
                    project: 4,
                    group: InboxGroup::Replies,
                    position: 7,
                    at: Wall::EPOCH,
                }]),
                needs: document().needs,
                updates: Count { value: 1, limited: false },
                results_seen: 0,
                replies_seen: list([]),
                more: false,
            }))),
        },
    );
    let id = d.objects().find(ObjectKey::Task(9)).unwrap();
    let mut bindings = JigBindings::new(64, CLIENT.text);
    let tree = render(&d, id, words, &mut bindings);
    assert!(contains(&tree, b"Live dependency"));
    let link = tree
        .nodes()
        .iter()
        .find(|n| {
            n.role == Role::Link && n.name.as_deref().is_some_and(|name| name.starts_with(b"Open at first reply"))
        })
        .unwrap()
        .binding
        .unwrap();
    assert_eq!(
        bindings.address(link.token),
        Some(crate::address::Address::Task { task: 9, section: crate::address::TaskSection::Activity })
    );
    assert_eq!(decode(&mut bindings, link.token, Act::Press), Some(Action::OpenChat { task: 9 }));
    let mut root = chip();
    root.phase = Phase::Waiting;
    *root.details = ChipPhase::Waiting { dependencies: list([9, 9]), hold: None };
    let mut b = Builder::new(&limits());
    b.open(Role::Page);
    bindings.begin();
    cards::chip::chip(&mut b, &d.view(), &root, words, &mut bindings);
    b.close();
    let tree = b.finish();
    super::cards_access::check(&tree);
    assert!(contains(&tree, b"Using tool actual tool"));
    assert_eq!(tree.nodes().iter().filter(|n| n.role == Role::Region).count(), 2);
    bindings.begin();
    assert_eq!(decode(&mut bindings, link.token, Act::Press), None);
}

fn impact() -> Impact {
    Impact {
        tasks: PhaseCounts { ended: 0, waiting: 1, running: 2, held: 3, closing: 4, done: 5, failed: 6, cancelled: 7 },
        live_runs: 8,
        proposals: 9,
    }
}
fn escalation_doc(reason: HoldReason, acts: &[ActKind]) -> Escalation {
    let mut task = chip();
    task.details = Box::new(ChipPhase::Held {
        reason: reason.clone(),
        since: Wall::EPOCH,
        revision: Revision { value: 4 },
        was: None,
    });
    let release = match &reason {
        HoldReason::Tries { .. } | HoldReason::Stopped { .. } | HoldReason::Stalled { .. } => ReleaseEffect::RunAgain,
        HoldReason::EffectUncertain { .. } => ReleaseEffect::Uncertain,
        _ => ReleaseEffect::Judged,
    };
    Escalation {
        task,
        hold: Hold {
            placed: None,
            revision: Revision { value: 4 },
            reason,
            before: Phase::Closing,
            since: Wall::EPOCH,
            result: Some(ResultValue::Report(DocumentText::complete(text(b"kept closing result")))),
            spend: list([
                SpendLine { task: 8, attempt: Some(2), spent: 7 },
                SpendLine { task: 9, attempt: None, spent: 3 },
            ]),
        },
        steering: Box::new(Steering {
            person_revision: Some(Revision { value: 12 }),
            taken_by: None,
            addressed: Some(Recipient::Role { project: 4, role: 2 }),
            revision: Revision { value: 6 },
            spec: specification(text(b"held specification")),
            instructions: Some(DocumentText::complete(text(b"held instructions"))),
            wake: OwnerWakePolicy {
                words: WakeRule::Immediate,
                notices: WakeRule::Never,
                news: WakeRule::Never,
                results: ResultsWake::Each,
                questions: false,
                answers: false,
                timers: false,
            },
            dependencies: list([9]),
            authority: owner_authority(),
            budget: Budget { total: 10, own: 10, ended: 0, reserved: 0, left: 0, funder: Actor::Party(1) },
        }),
        impact: impact(),
        waiting: Recipient::Task(1),
        trail: list([Passed {
            by: Actor::Party(3),
            at: Wall::EPOCH,
            reason: text(b"previous holder reason"),
            lacking: None,
        }]),
        acts: offered_acts(acts, release),
        ending: None,
    }
}
fn dialog(d: &Fixture, words: &crate::words::Words, bindings: &mut JigBindings) -> Tree {
    bindings.begin();
    let mut b = Builder::new(&limits());
    b.open(Role::Page);
    crate::confirmation::confirmation(&mut b, &d.view(), words, bindings);
    b.close();
    let tree = b.finish();
    super::cards_access::check(&tree);
    assert!(tree.len() <= crate::confirmation_nodes(&CLIENT).unwrap().checked_add(1).unwrap());
    tree
}
pub(super) fn task_doc(acts: &[ActKind]) -> Task {
    Task {
        dependency_facts: List::with_capacity(0),
        kept_account: None,
        run_wait: None,
        settings: None,
        llm: None,
        label: 0,
        flags: TaskFlags { ended_by: EndedBy::Result, conversation: false, listed: true, retention: Retention::Kept },
        last_activity: Wall::EPOCH,
        waits_on: WaitsOn::Nothing,
        chip: chip(),
        goal: Some(8),
        priority: Some(1),
        lineage_chips: list([]),
        inputs: list([]),
        project: 4,
        requester: OwnerRequester::Person(1),
        lineage: list([]),
        spec: specification(text(b"whole task specification")),
        spec_revision: Revision { value: 6 },
        person_revision: None,
        taken_by: None,
        instructions: Some(DocumentText::complete(text(b"whole task instructions"))),
        wake: OwnerWakePolicy {
            words: WakeRule::Immediate,
            notices: WakeRule::Never,
            news: WakeRule::Never,
            results: ResultsWake::Each,
            questions: false,
            answers: false,
            timers: false,
        },
        subscriptions: Subscriptions { entries: List::with_capacity(0) },
        result_contract: TaskContractDocument::Report { words: 32 },
        dependencies: list([]),
        cut: false,
        references: list([]),
        resources: list([]),
        budget: OwnerBudget {
            numbers: OwnerNumbers { budget: 20, spent: 0, spent_below: 0, reserved: 0 },
            left: 20,
            funder: Funder::Pool { project: 4, person: 1, period: 3 },
            deadline: None,
        },
        authority: owner_authority(),
        hold: Some(TaskHold::Document(Box::new(escalation_doc(HoldReason::Budget { spent: 10, limit: 10 }, &[]).hold))),
        proposals: list([]),
        activity: 0,
        changed: Wall::EPOCH,
        acts: offered_acts(acts, ReleaseEffect::Judged),
        result: None,
        delegates: list([LiveDelegate { chip: chip(), takeover_budget: 13 }]),

        context: Some(Box::new(TaskContext {
            escalation: None,
            parts: list([]),
            closing: None,
            spend: list([]),
            runtime: Runtime::Held { was: None },
            impact: impact(),
            writer_slots: list([]),
        })),
    }
}
fn input(tree: &Tree, label: &[u8], value: &[u8], bindings: &mut JigBindings) -> Action {
    let node = tree.nodes().iter().find(|n| n.name.as_deref() == Some(label) && n.binding.is_some()).unwrap();
    decode(bindings, node.binding.unwrap().token, Act::Input { value: Box::from(value) }).unwrap()
}
fn escalation_reasons_confirmation_choices_changed_revisions_and_notices_keep_semantic_facts(
    words: &crate::words::Words,
) {
    use crate::confirmation::amendment::deadline;
    use ActKind::{Amend, Cancel, Release};
    let mut bindings = JigBindings::new(limits().bindings, CLIENT.text);
    let part = Part::Hold { connector: 2, task: 8 };
    let reasons = [
        (
            HoldReason::Tries {
                failures: list([AttemptFailure {
                    attempt: 2,
                    class: FailureClass::Permanent,
                    reason: text(b"attempt refusal"),
                    at: Wall::EPOCH,
                }]),
            },
            vec![Release, Amend, Cancel],
            b"attempt refusal".as_slice(),
        ),
        (
            HoldReason::Stalled { since: Wall::EPOCH, waiting: text(b"the person") },
            vec![Release, Amend, Cancel],
            b"the person".as_slice(),
        ),
        (
            HoldReason::Dependency { task: 9, how: Ending::Failed { reason: Some(text(b"dependency refusal")) } },
            vec![Amend, Cancel],
            b"dependency refusal".as_slice(),
        ),
        (HoldReason::Drift { part: part.clone() }, vec![Release, Cancel], b"system changed".as_slice()),
        (HoldReason::EffectFailed { part: part.clone() }, vec![Release, Cancel], b"failed for good".as_slice()),
        (HoldReason::EffectUncertain { part: part.clone() }, vec![Release, Cancel], b"may or may not".as_slice()),
        (HoldReason::Budget { spent: 10, limit: 10 }, vec![Amend, Cancel], b"spent 10".as_slice()),
        (HoldReason::Deadline { at: Wall::EPOCH }, vec![Amend, Cancel], b"past deadline".as_slice()),
        (
            HoldReason::Stopped { by: super::cards::party(), at: Wall::EPOCH },
            vec![Release, Amend, Cancel],
            b"stopped by Pat".as_slice(),
        ),
        (
            HoldReason::Resource { resource: part.clone(), holder: Actor::Task(9) },
            vec![Release, Amend, Cancel],
            b"past its bound".as_slice(),
        ),
        (HoldReason::Pool { part: part.clone() }, vec![Release, Cancel], b"pool lost".as_slice()),
        (HoldReason::Procedure { part: part.clone() }, vec![Release, Amend, Cancel], b"procedure needs".as_slice()),
    ];
    for (reason, acts, expected) in reasons {
        escalation_row_scenario(reason, &acts, expected, words, &mut bindings);
    }
    confirmation_act_scenarios(words, &mut bindings);
    goal_dialog_scenario(words, &mut bindings);
    notice_scenarios(words);
    assert_eq!(deadline(b"1970-01-01 00:00"), Some(Wall::EPOCH));
    assert_eq!(deadline(b"2000-02-29 12:34").unwrap().as_nanos(), 951_827_640_000_000_000);
    assert!(deadline(b"2100-02-29 00:00").is_none());
    assert!(deadline(b"2000-02-30 00:00").is_none());
    assert!(deadline(b"1969-12-31 23:59").is_none());
    assert!(deadline(b"9999-12-31 00:00").is_none());
}
const CONFIRMATION_ACTS: [(ActKind, bool); 18] = [
    (ActKind::Message, false),
    (ActKind::Take, false),
    (ActKind::HandBack, false),
    (ActKind::Release, false),
    (ActKind::LeaveHeld, false),
    (ActKind::Amend, false),
    (ActKind::Amend, true),
    (ActKind::Stop, false),
    (ActKind::Cancel, false),
    (ActKind::Move, false),
    (ActKind::Reorder, false),
    (ActKind::Reorder, false),
    (ActKind::Seen, false),
    (ActKind::Adopt, false),
    (ActKind::ChangePolicy, false),
    (ActKind::SetRoles, false),
    (ActKind::MakeService, false),
    (ActKind::SetPool, false),
];
fn confirmation_act_scenarios(words: &crate::words::Words, bindings: &mut JigBindings) {
    for (act, composed) in CONFIRMATION_ACTS {
        let task = task_doc(&[act]);
        let (mut d, id) = loaded(ObjectKey::Task(8), ReadValue::Task(Box::new(task)));
        let action = if composed {
            Action::AmendRelease { object: id.token() }
        } else {
            Action::Intend { object: id.token(), act }
        };
        drive(&mut d, Event::Act { action });
        let tree = dialog(&d, words, bindings);
        let phrase =
            if composed { crate::phrases::jig::JigPhrase::ActAmendRelease } else { cards::common::act_phrase(act) };
        assert!(contains(&tree, &words.say(phrase, &[])));
        if act == ActKind::Cancel {
            assert!(contains(&tree, b"Dangerous action"));
            assert!(contains(&tree, b"Effects not yet begun will be withdrawn; begun effects will settle"));
            assert!(tree.nodes().iter().any(|n| n.role == Role::Button
                && n.styles.as_slice().contains(&crate::JigStyle::Danger.token())
                && n.states.as_slice().contains(&State::Disabled)));
            let edit = input(&tree, b"Your reason", b"stop it", bindings);
            drive(&mut d, Event::Act { action: edit });
            let tree = dialog(&d, words, bindings);
            assert_eq!(press(&tree, b"Confirm", bindings), Action::Confirm);
        }
        if act == ActKind::Reorder {
            let change = input(&tree, b"New goal priority", b"3", bindings);
            drive(&mut d, Event::Act { action: change });
            assert_eq!(d.confirmation().unwrap().choices.priority, Some(3));
        }
    }
    for act in [ActKind::Accept, ActKind::Reject, ActKind::PassUp] {
        let mut p = proposal(ProposalAction::Batch { tasks: list([]), own: true });
        p.acts = list([Offered { effect: None, act, admission: Admission::Proposed { to: Recipient::Task(2) } }]);
        let (mut d, id) = loaded(ObjectKey::Proposal(p.key), ReadValue::Proposal(Box::new(p)));
        drive(&mut d, Event::Act { action: Action::Intend { object: id.token(), act } });
        let tree = dialog(&d, words, bindings);
        assert!(contains(&tree, b"will be proposed"));
        assert!(contains(&tree, b"Waiting for task 2"));
        if act == ActKind::Accept {
            assert!(contains(&tree, b"accepted tasks will be yours"));
            assert!(contains(&tree, b"Funds 10; 20 left now, 10 after"));
        }
    }
    let p = person(PersonContract::Words);
    let (mut d, id) = loaded(ObjectKey::PersonTask(8), ReadValue::PersonTask(Box::new(p)));
    drive(&mut d, Event::Act { action: Action::Intend { object: id.token(), act: ActKind::Answer } });
    let tree = dialog(&d, words, bindings);
    assert!(tree.nodes().iter().any(|n| n.role == Role::TextArea && n.name.as_deref() == Some(b"Your answer")));
    let choose = press(&tree, b"Cannot do it", bindings);
    drive(&mut d, Event::Act { action: choose });
    assert!(d.confirmation().unwrap().choices.cannot);
    for act in [ActKind::EditNote, ActKind::DeleteNote] {
        let note = Note {
            number: 1,
            revision: Revision { value: 2 },
            scope: NoteScope::Project(4),
            description: DocumentText::complete(text(b"note title")),
            body: DocumentText::complete(text(b"note body")),
            writer: NoteAuthor::Party { party: 1 },
            tasks: list([]),
            changed: Some(Wall::EPOCH),
            acts: offered(act),
            cut: false,
        };
        let (mut d, id) = note_loaded(note);
        drive(&mut d, Event::Act { action: Action::Intend { object: id.token(), act } });
        let tree = dialog(&d, words, bindings);
        assert!(contains(&tree, b"note title"));
        if act == ActKind::EditNote {
            assert!(tree.nodes().iter().any(|n| n.value.as_ref().is_some_and(|v| v.text.as_ref() == b"note body")));
        }
    }
    changed_dialog_scenario(words, bindings);
}
fn goal_dialog_scenario(words: &crate::words::Words, bindings: &mut JigBindings) {
    use crate::address::{Address, Place, ProjectSection};
    let (mut d, frame) = start();
    let doc = document();
    drive(&mut d, Watched { watch: frame, event: WatchEvent::Delivered(Delivery::Me(Box::new(doc))) });
    let out = drive(
        &mut d,
        Event::Went { place: Place::Jig(Address::Project { project: 4, section: ProjectSection::Goals }) },
    );
    let read = out
        .iter()
        .find_map(|r| match r {
            Request::Client {
                asked: jig_client::Event::Read { handle: read, read: jig_client::Readable::Jig(Read::Project { .. }) },
            } => Some(*read),
            _ => None,
        })
        .unwrap();
    let budget = Budget { total: 20, own: 0, ended: 0, reserved: 0, left: 20, funder: Actor::Party(1) };
    let project = Project {
        number: 4,
        name: text(b"One"),
        period: 1,
        spend_lines: list([]),
        policy: Policy {
            ceiling: authority(),
            period: 1,
            spend: 0,
            roles: list([]),
            requirements: list([]),
            deciders: list([]),
        },
        roles: list([]),
        spend: budget,
        pools: list([Pool { party: 1, period: 1, budget }]),
        resources: list([]),
        services: list([]),
        recurring: list([]),
        parts: list([]),
        acts: list([Offered {
            effect: None,
            act: ActKind::Reorder,
            admission: Admission::Proposed { to: Recipient::Role { project: 4, role: 2 } },
        }]),
    };
    drive(&mut d, ReadPage { read, result: ReadResult::Value(ReadValue::Project(Box::new(project))) });
    drive(&mut d, Event::Act { action: Action::Edit { field: Purpose::NewGoal, text: text(b"whole new goal") } });
    drive(
        &mut d,
        Event::Act {
            action: Action::GoalForm {
                form: crate::page::project::goals::GoalForm {
                    project: 4,
                    executor: OwnerExecutor::Agent { charter: 1 },
                    budget: 21,
                    deadline: Some(Wall::EPOCH),
                    priority: 2,
                },
            },
        },
    );
    drive(&mut d, Event::Act { action: Action::Submit { field: Purpose::NewGoal } });
    let tree = dialog(&d, words, bindings);
    assert!(contains(&tree, b"whole new goal"));
    assert!(contains(&tree, b"Your pool has 20 left; this needs 21"));
    assert!(contains(&tree, b"will be proposed"));
    assert!(contains(&tree, b"Charter 1"));
    assert_eq!(press(&tree, b"Confirm", bindings), Action::Confirm);
}
fn notice_scenarios(words: &crate::words::Words) {
    use crate::notices::kept::Kind;
    for kind in [
        Kind::Link(jig_client::Link::Offline),
        Kind::Link(jig_client::Link::Live),
        Kind::Answer { object: ObjectKey::Task(8), answer: Answer::Done(Outcome::Changed) },
        Kind::PageAnswer {
            answer: refusal_answer(Refusal::Authority {
                lacking: Lacking::Budget { needed: 40, left: 12 },
                proposable: None,
            }),
        },
        Kind::Unknown { object: ObjectKey::Escalation(8), act: ActKind::Release },
        Kind::PageUnknown { purpose: crate::page::IntentPurpose::CreateGoal },
        Kind::Refused(Limit::Objects),
    ] {
        let mut b = Builder::new(&limits());
        crate::notices::facts(&mut b, &kind, words, CLIENT.text);
        let tree = b.finish();
        super::cards_access::check(&tree);
        assert_eq!(tree.nodes()[0].role, Role::Status);
        assert!(tree.nodes()[0].name.is_some());
        assert!(tree.len() > 1);
    }
    let (mut d, _) = start();
    crate::notices::kept::show(&mut d, Kind::Refused(Limit::Objects));
    let mut b = Builder::new(&limits());
    let mut bindings = JigBindings::new(limits().bindings, CLIENT.text);
    crate::notices::notices(&mut b, &d.view(), words, &mut bindings);
    let tree = b.finish();
    super::cards_access::check(&tree);
    assert!(contains(&tree, b"Working set limit"));
    let dismiss = press(&tree, b"Dismiss", &mut bindings);
    drive(&mut d, Event::Act { action: dismiss });
    assert!(d.notices().ids().is_empty());
}

fn note_loaded(note: Note) -> (Fixture, Id<Object>) {
    use crate::address::{Address, Place, ProjectSection};
    let number = note.number;
    let (mut d, frame) = start();
    drive(&mut d, Watched { watch: frame, event: WatchEvent::Delivered(Delivery::Me(Box::new(document()))) });
    let out = drive(
        &mut d,
        Event::Went { place: Place::Jig(Address::Project { project: 4, section: ProjectSection::Notes }) },
    );
    let read = out
        .iter()
        .find_map(|r| match r {
            Request::Client {
                asked: jig_client::Event::Read { handle: read, read: jig_client::Readable::Jig(Read::Notes { .. }) },
            } => Some(*read),
            _ => None,
        })
        .unwrap();
    drive(
        &mut d,
        ReadPage {
            read,
            result: ReadResult::Value(ReadValue::Notes(Box::new(NotePage {
                cut: false,
                rows: list([note]),
                before: None,
                after: None,
            }))),
        },
    );
    let id = d.objects().find(ObjectKey::Note { project: 4, note: number }).unwrap();
    (d, id)
}

#[expect(
    clippy::match_like_matches_macro,
    reason = "ordinary scenario patterns use matches with an explicit catch-all; the macro is disallowed"
)]
fn escalation_row_scenario(
    reason: HoldReason,
    acts: &[ActKind],
    expected: &[u8],
    words: &crate::words::Words,
    bindings: &mut JigBindings,
) {
    let row = escalation_doc(reason, acts);
    let (mut d, id) = loaded(ObjectKey::Escalation(8), ReadValue::Escalation(Box::new(row.clone())));
    let tree = render(&d, id, words, bindings);
    assert!(contains(&tree, expected));
    assert!(contains(&tree, b"kept closing result"));
    assert!(contains(&tree, b"Task 8, attempt 2: spent 7"));
    assert!(contains(&tree, b"Task 9: spent 3"));
    assert!(contains(&tree, b"previous holder reason"));
    for act in acts {
        assert!(
            tree.nodes().iter().any(|n| n.role == Role::Button
                && n.name.as_deref() == Some(&words.say(cards::common::act_phrase(*act), &[])))
        );
    }
    let action = if acts[0] == ActKind::Amend {
        Action::AmendRelease { object: id.token() }
    } else {
        Action::Intend { object: id.token(), act: acts[0] }
    };
    drive(&mut d, Event::Act { action });
    let tree = dialog(&d, words, bindings);
    assert!(tree.nodes().iter().any(|n| n.role == Role::Dialog && n.name.is_some()));
    if match row.hold.reason {
        HoldReason::EffectUncertain { .. } => true,
        _ => false,
    } {
        assert!(contains(&tree, b"engine will do as you choose"));
        let choose = press(&tree, b"Release without making the effect again; tell its executor so", bindings);
        drive(&mut d, Event::Act { action: choose });
        assert_eq!(d.confirmation().unwrap().choices.again, Some(false));
        let tree = dialog(&d, words, bindings);
        let send = press(&tree, b"Confirm", bindings);
        let out = drive(&mut d, Event::Act { action: send });
        assert!(out.iter().any(|r| match r {
            Request::Client { asked: jig_client::Event::Send { request: jig_client::Ask::Jig(ask), .. } } =>
                match **ask {
                    Ask::Release { again: Some(false), .. } => true,
                    _ => false,
                },
            _ => false,
        }));
    }
    if acts[0] == ActKind::Amend {
        assert!(contains(&tree, b"held specification"));
        assert!(contains(&tree, b"Hold revision 4"));
        assert_eq!(d.confirmation().unwrap().release_revision, Some(Revision { value: 4 }));
        let initial_writes = d.confirmation().unwrap().writes;
        let written = tree
            .nodes()
            .iter()
            .find(|n| n.name.as_deref() == Some(b"New instructions"))
            .unwrap()
            .value
            .as_ref()
            .unwrap()
            .written;
        assert_eq!(written, initial_writes);
        let change = input(&tree, b"New budget", b"25", bindings);
        drive(&mut d, Event::Act { action: change });
        assert_eq!(d.confirmation().unwrap().choices.change.as_ref().unwrap().authority.as_ref().unwrap().spend, 25);
        let tree = dialog(&d, words, bindings);
        let change = input(&tree, b"New instructions", b"try another way", bindings);
        drive(&mut d, Event::Act { action: change });
        let open = d.confirmation().unwrap();
        assert_eq!(open.choices.change.as_ref().unwrap().instructions.as_ref().unwrap().bytes(), b"try another way");
        assert_eq!(open.choices.change.as_ref().unwrap().authority.as_ref().unwrap().spend, 25);
        assert_eq!(open.revision, Some(Revision { value: 6 }));
        let tree = dialog(&d, words, bindings);
        let change = input(&tree, b"Address to role number in this project", b"7", bindings);
        drive(&mut d, Event::Act { action: change });
        assert_eq!(
            d.confirmation().unwrap().choices.change.as_ref().unwrap().addressed,
            Some(AddressChange { revision: Revision { value: 12 }, address: PersonAddress::Role(7) })
        );
    }
    if expected == b"spent 10" {
        let initial_writes = d.confirmation().unwrap().writes;
        drive(&mut d, Event::Act { action: Action::Dismiss });
        drive(&mut d, Event::Act { action: Action::AmendRelease { object: id.token() } });
        drive(&mut d, Event::Act { action: Action::Edit { field: Purpose::Reason, text: text(b"") } });
        assert!(d.confirmation().unwrap().writes > initial_writes);
        let tree = dialog(&d, words, bindings);
        let value = tree
            .nodes()
            .iter()
            .find(|n| n.name.as_deref() == Some(b"New instructions"))
            .unwrap()
            .value
            .as_ref()
            .unwrap();
        assert_eq!(value.text.as_ref(), b"held instructions");
        assert_eq!(value.written, d.confirmation().unwrap().writes);
    }
}

fn changed_dialog_scenario(words: &crate::words::Words, bindings: &mut JigBindings) {
    let row = escalation_doc(HoldReason::Budget { spent: 10, limit: 10 }, &[ActKind::Release]);
    let (mut d, id, _watch) = loaded_with_watch(ObjectKey::Escalation(8), ReadValue::Escalation(Box::new(row.clone())));
    drive(&mut d, Event::Act { action: Action::Intend { object: id.token(), act: ActKind::Release } });
    let out = drive(&mut d, Event::Act { action: Action::Confirm });
    let request = out
        .iter()
        .find_map(|r| match r {
            Request::Client { asked: jig_client::Event::Send { key: request, .. } } => Some(*request),
            _ => None,
        })
        .unwrap();
    let out = drive(
        &mut d,
        AnsweredByEngine {
            request,
            answer: refusal_answer(Refusal::Moved { now: Revision { value: 5 }, what: Moved::Hold { reason: None } }),
        },
    );
    let mut moved = row;
    moved.hold.revision.value = 5;
    let read = out
        .iter()
        .find_map(|r| match r {
            Request::Client { asked: jig_client::Event::Read { handle: read, .. } } => Some(*read),
            _ => None,
        })
        .unwrap();
    drive(&mut d, Event::Act { action: Action::Intend { object: id.token(), act: ActKind::Release } });
    drive(&mut d, ReadPage { read, result: ReadResult::Value(ReadValue::Escalation(Box::new(moved))) });
    let tree = dialog(&d, words, bindings);
    assert!(contains(&tree, b"revision this action names changed"));
    assert!(!tree.nodes().iter().any(|n| n.name.as_deref() == Some(b"Confirm") && n.binding.is_some()));
}

#[path = "cards_sizes.rs"]
mod sizes;

#[test]
fn jig_object_cards_hold_actions_confirmations_and_notices() {
    let words = words();
    whole_object_cards_keep_facts_while_confirmation_request_refusal_and_leaving_change(words);
    person_contracts_and_result_values_keep_choices_slots_and_full_payloads(words);
    proposal_kinds_chip_phases_and_live_replies_keep_facts_and_slots(words);
    escalation_reasons_confirmation_choices_changed_revisions_and_notices_keep_semantic_facts(words);
    sizes::scenario(words);
    closing_connectors_use_their_owners_words(words);
    person_chip_preserves_absent_readiness(words);
    authority_refusal_uses_only_supplied_proposal_eligibility(words);
    collective_recipients_name_every_supplied_role_in_order(words);
}

fn closing_connectors_use_their_owners_words(words: &crate::words::Words) {
    let mut b = Builder::new(&limits());
    let mut bindings = JigBindings::new(16, CLIENT.text);
    b.open(Role::Page);
    cards::common::closing(
        &mut b,
        &Closing { delegates: 2, connectors: Some(list([7, 11])), stage: Some(ClosingStage::Effects), ending: None },
        words,
        &mut bindings,
    );
    b.close();
    let slots = b.slots(0);
    assert_eq!(slots.len(), 2);
    for slot in &slots {
        let connector = match bindings.part(slot.reference.token).unwrap() {
            Part::Connector { connector } => *connector,
            _ => panic!("a name-only connector part"),
        };
        assert_eq!(slot.reference.owner, (crate::Owner::Application { number: connector }).opaque());
        let mut own = Builder::new(&limits());
        own.open(Role::Region);
        own.text(skein_ui::Run::Plain, if connector == 7 { b"amber" } else { b"blue" });
        own.close();
        b.splice(slot.id, own);
    }
    let tree = b.finish();
    for fact in [b"Closing: 2 delegates".as_slice(), b"amber", b"blue", b"'s effects settling"] {
        assert!(contains(&tree, fact));
    }
    assert!(!contains(&tree, b"1 effects"));
    assert!(!contains(&tree, b"2 effects"));
}

fn person_chip_preserves_absent_readiness(words: &crate::words::Words) {
    for since in [None, Some(Wall::EPOCH)] {
        let (d, _) = start();
        let mut value = chip();
        value.phase = Phase::Active;
        *value.details = ChipPhase::Person { addressed: Recipient::Party(1), taken: None, since };
        let mut b = Builder::new(&limits());
        let mut bindings = JigBindings::new(16, CLIENT.text);
        b.open(Role::Page);
        cards::chip::chip(&mut b, &d.view(), &value, words, &mut bindings);
        b.close();
        let tree = b.finish();
        assert_eq!(contains(&tree, b"Since"), since.is_some());
    }
}

fn authority_refusal_uses_only_supplied_proposal_eligibility(words: &crate::words::Words) {
    for proposable in [None, Some(true), Some(false)] {
        let mut b = Builder::new(&limits());
        b.open(Role::Page);
        words.refusal(&mut b, &Refusal::Authority { lacking: Lacking::Approver, proposable }, CLIENT.text);
        b.close();
        let tree = b.finish();
        assert!(contains(&tree, b"Proposal approval authority is missing"));
        assert_eq!(contains(&tree, b"Nobody in this project may accept it"), proposable == Some(false));
    }
}

fn collective_recipients_name_every_supplied_role_in_order(words: &crate::words::Words) {
    let mut b = Builder::new(&limits());
    b.open(Role::Page);
    cards::common::recipient(&mut b, &Recipient::Roles { project: 4, roles: list([7, 2]) }, words);
    b.close();
    let tree = b.finish();
    let names: Vec<_> =
        tree.nodes().iter().flat_map(|node| node.text.as_slice().iter().map(|run| run.text.as_ref())).collect();
    assert_eq!(names, [b"Waiting at policy roles in project 4".as_slice(), b"Role 7", b"Role 2"]);
}

#[test]
fn approved_and_failed_results_show_only_their_supplied_owner_payloads() {
    let (domain, _) = start();
    let words = words();
    for explanation in [None, Some(text(b"")), Some(text(b"exact explanation"))] {
        let value = ResultValue::Approved {
            state: Box::new([0, 255, 1]),
            words: explanation.clone().map(DocumentText::complete),
        };
        let mut builder = Builder::new(&limits());
        builder.open(Role::Page);
        let mut bindings = JigBindings::new(64, CLIENT.text);
        cards::result::value_nodes(&mut builder, &domain.view(), &value, words, &mut bindings);
        builder.close();
        let tree = builder.finish();
        assert!(contains(&tree, &[0, 255, 1]));
        assert!(!contains(&tree, document().party.name.bytes()));
        if let Some(explanation) = explanation
            && !explanation.bytes().is_empty()
        {
            assert!(contains(&tree, explanation.bytes()));
        }
    }
    let mut builder = Builder::new(&limits());
    builder.open(Role::Page);
    let mut bindings = JigBindings::new(64, CLIENT.text);
    cards::result::value_nodes(
        &mut builder,
        &domain.view(),
        &ResultValue::Failure(DocumentText::complete(text(b"cannot complete"))),
        words,
        &mut bindings,
    );
    builder.close();
    let tree = builder.finish();
    assert!(contains(&tree, b"Failed"));
    assert!(contains(&tree, b"cannot complete"));
}

fn specification(words: Text) -> SpecificationDocument {
    SpecificationDocument::complete(Specification {
        words,
        parameters: List::with_capacity(0),
        inputs: List::with_capacity(0),
    })
}

#[test]
fn whole_specification_words_edit_preserves_parameters_and_input_identities() {
    let mut row = task_doc(&[ActKind::Amend]);
    row.spec.specification.parameters = list([
        ParameterDocument::Number { name: u32::MAX, value: u64::MAX },
        ParameterDocument::Bytes { name: 8, value: DocumentText::complete(text(UTF8_TEXTS[0])) },
        ParameterDocument::Resource { name: 9, connector: u16::MAX, resource: u64::MAX },
    ]);
    row.spec.specification.inputs = list([u64::MAX, 0]);
    let expected = row.spec.whole().unwrap();
    let (mut domain, id) = loaded(ObjectKey::Task(8), ReadValue::Task(Box::new(row)));
    drive(&mut domain, Event::Act { action: Action::Intend { object: id.token(), act: ActKind::Amend } });
    drive(&mut domain, Event::Act { action: Action::Edit { field: Purpose::Reason, text: text(b"") } });
    let words = words();
    let mut bindings = JigBindings::new(256, CLIENT.text);
    let tree = dialog(&domain, words, &mut bindings);
    assert!(contains(&tree, UTF8_TEXTS[0]));
    assert!(contains(&tree, b"Parameter 4294967295 has value 18446744073709551615"));
    assert!(contains(&tree, b"Parameter 9 names connector 65535 resource 18446744073709551615"));
    assert!(contains(&tree, b"Input task 18446744073709551615"));
    drive(&mut domain, Event::Act { action: input(&tree, b"New specification", b"replacement", &mut bindings) });
    let out = drive(&mut domain, Event::Act { action: Action::Confirm });
    let sent = out
        .iter()
        .find_map(|request| match request {
            Request::Client { asked: jig_client::Event::Send { request: jig_client::Ask::Jig(ask), .. } } => {
                Some(&**ask)
            }
            _ => None,
        })
        .expect("whole amendment sent");
    let Ask::Amend { change, .. } = sent else { panic!("amendment") };
    let replacement = change.spec.as_ref().expect("whole specification");
    assert_eq!(replacement.words.bytes(), b"replacement");
    assert_eq!(replacement.parameters, expected.parameters);
    assert_eq!(replacement.inputs, expected.inputs);
}

#[test]
fn cut_specification_is_shown_and_cannot_be_reconstructed_by_the_words_editor() {
    let mut row = task_doc(&[ActKind::Amend]);
    row.spec.cut = true;
    let (mut domain, id) = loaded(ObjectKey::Task(8), ReadValue::Task(Box::new(row)));
    let words = words();
    let mut builder = Builder::new(&limits());
    builder.open(Role::Page);
    crate::specification::document(
        &mut builder,
        &{
            let mut document = SpecificationDocument::complete(Specification {
                words: text(b"partial"),
                parameters: list([]),
                inputs: list([]),
            });
            document.cut = true;
            document
        },
        words,
    );
    builder.close();
    assert!(contains(&builder.finish(), b"The specification is incomplete"));
    drive(&mut domain, Event::Act { action: Action::Intend { object: id.token(), act: ActKind::Amend } });
    drive(&mut domain, Event::Act { action: Action::Edit { field: Purpose::Reason, text: text(b"") } });
    let mut bindings = JigBindings::new(256, CLIENT.text);
    let tree = dialog(&domain, words, &mut bindings);
    assert!(!tree.nodes().iter().any(|node| node.name.as_deref() == Some(b"New specification")));
    assert!(drive(&mut domain, Event::Act { action: Action::Confirm }).is_empty());
}

#[test]
fn person_task_cards_preserve_numeric_takers_without_inventing_names() {
    let words = words();
    for supplied_name in [None, Some(text(b"actual taker"))] {
        let mut value = person(PersonContract::Words);
        value.revision = Revision { value: u64::MAX };
        value.addressed = Recipient::Role { project: 4, role: u32::MAX };
        value.task.executor = Executor::Role { project: 4, role: u32::MAX };
        value.taken = Some(TakenBy { number: u64::MAX, name: supplied_name.clone() });
        let (domain, object, _) = loaded_with_watch(ObjectKey::PersonTask(8), ReadValue::PersonTask(Box::new(value)));
        let mut bindings = JigBindings::new(16, CLIENT.text);
        let tree = render(&domain, object, words, &mut bindings);
        assert!(contains(&tree, b"Waiting for project 4, role 4294967295"));
        assert_eq!(contains(&tree, b"Taken by party 18446744073709551615"), supplied_name.is_none());
        assert_eq!(contains(&tree, b"Taken by actual taker"), supplied_name.is_some());
        assert!(!contains(&tree, b"Taken by Pat"));
    }
}

#[test]
fn retained_task_facts_keep_person_revision_taker_and_full_executor_identity_independent() {
    for (executor, person_revision, taken_by) in [
        (Executor::Role { project: 4, role: u32::MAX }, Some(Revision { value: u64::MAX }), Some(u64::MAX - 1)),
        (Executor::Procedure { connector: 2, kind: u32::MAX }, None, None),
    ] {
        let mut value = task_doc(&[ActKind::Amend]);
        value.goal = None;
        value.chip.executor = executor;
        value.person_revision = person_revision;
        value.taken_by = taken_by;
        let (domain, object, _) = loaded_with_watch(ObjectKey::Task(8), ReadValue::Task(Box::new(value)));
        let Some(crate::objects::Body::Task(retained)) = &domain.objects().get(object).unwrap().body else {
            panic!("the actual task document remains retained");
        };
        assert_eq!(retained.chip.executor, executor);
        assert_eq!(retained.person_revision, person_revision);
        assert_eq!(retained.taken_by, taken_by);
        assert_eq!(retained.spec_revision, Revision { value: 6 });
        assert_eq!(retained.hold.as_ref().unwrap().revision(), Revision { value: 4 });
    }
}

#[test]
fn specification_documents_keep_independent_text_and_list_loss_and_block_whole_edits() {
    for mask in 0_u8..8 {
        let mut row = task_doc(&[ActKind::Amend]);
        row.spec.specification.parameters = list([ParameterDocument::Bytes {
            name: 8,
            value: DocumentText { bytes: text(UTF8_TEXTS[0]), cut: mask & 2 != 0 },
        }]);
        row.spec.specification.words.cut = mask & 1 != 0;
        row.spec.cut = mask & 4 != 0;
        row.spec.specification.inputs = list([u64::MAX]);
        assert_eq!(row.spec.has_loss(), mask != 0);
        assert_eq!(row.spec.whole().is_some(), mask == 0);
        let words = words();
        let mut builder = Builder::new(&limits());
        crate::specification::document(&mut builder, &row.spec, words);
        let tree = builder.finish();
        assert!(contains(&tree, UTF8_TEXTS[0]));
        assert_eq!(contains(&tree, b"Only part of this text was supplied"), mask & 3 != 0);
        let (mut domain, id) = loaded(ObjectKey::Task(8), ReadValue::Task(Box::new(row)));
        drive(&mut domain, Event::Act { action: Action::Intend { object: id.token(), act: ActKind::Amend } });
        drive(&mut domain, Event::Act { action: Action::Edit { field: Purpose::Reason, text: text(b"") } });
        let mut bindings = JigBindings::new(256, CLIENT.text);
        let tree = dialog(&domain, words, &mut bindings);
        if mask != 0 {
            assert!(!tree.nodes().iter().any(|node| node.name.as_deref() == Some(b"New specification")));
            assert!(drive(&mut domain, Event::Act { action: Action::Confirm }).is_empty());
        } else {
            drive(
                &mut domain,
                Event::Act { action: input(&tree, b"New specification", b"replacement", &mut bindings) },
            );
            let out = drive(&mut domain, Event::Act { action: Action::Confirm });
            assert!(out.iter().any(|request| match request {
                Request::Client { asked: jig_client::Event::Send { request: jig_client::Ask::Jig(ask), .. } } =>
                    match &**ask {
                        Ask::Amend { change, .. } =>
                            change.spec.as_ref().is_some_and(|spec| spec.words.bytes() == b"replacement"
                                && spec.inputs == list([u64::MAX])
                                && spec.parameters == list([Parameter::Bytes { name: 8, value: text(UTF8_TEXTS[0]) }])),
                        _ => false,
                    },
                _ => false,
            }));
        }
    }
}

#[test]
#[expect(clippy::match_like_matches_macro, reason = "scenario matching stays explicit; matches is disallowed")]
fn note_documents_keep_each_text_loss_and_reference_loss_without_reconstructing_corrections() {
    for mask in 0_u8..8 {
        for act in [ActKind::EditNote, ActKind::DeleteNote] {
            let note = Note {
                number: 77,
                revision: Revision { value: 123 },
                scope: NoteScope::Task { project: 1, task: 900 },
                description: DocumentText { bytes: text(b"note title"), cut: mask & 1 != 0 },
                body: DocumentText { bytes: text(b"note body"), cut: mask & 2 != 0 },
                writer: NoteAuthor::Party { party: 1 },
                tasks: list([91]),
                changed: Some(Wall::EPOCH),
                acts: offered(act),
                cut: mask & 4 != 0,
            };
            let (mut domain, id) = note_loaded(note);
            let words = words();
            let mut bindings = JigBindings::new(256, CLIENT.text);
            let mut builder = Builder::new(&limits());
            cards::card(&mut builder, &domain.view(), id, &cards::Place::Elsewhere, words, &mut bindings);
            let tree = builder.finish();
            assert_eq!(contains(&tree, b"Only part of this text was supplied"), mask & 3 != 0);
            assert_eq!(contains(&tree, b"This note has more references"), mask & 4 != 0);
            drive(&mut domain, Event::Act { action: Action::Intend { object: id.token(), act } });
            let tree = dialog(&domain, words, &mut bindings);
            let out = drive(&mut domain, Event::Act { action: Action::Confirm });
            if act == ActKind::EditNote && mask != 0 {
                assert!(tree.nodes().iter().all(|node| node.value.is_none()));
                assert!(out.iter().all(|request| match request {
                    Request::Client { asked: jig_client::Event::Send { .. } } => false,
                    _ => true,
                }));
            } else {
                assert!(out.iter().any(|request| match request {
                    Request::Client { asked: jig_client::Event::Send { .. } } => true,
                    _ => false,
                }));
            }
        }
    }
}

#[path = "amendment_presence.rs"]
mod amendment_presence;
#[test]
fn equal_number_proposal_makers_keep_distinct_card_keys_words_and_decoding() {
    let words = words();
    let mut shown = Vec::new();
    for (maker, kind, label) in [
        (Proposer::Task(u64::MAX), 1, b"By task 18446744073709551615".as_slice()),
        (Proposer::Party(u64::MAX), 29, b"By party 18446744073709551615".as_slice()),
    ] {
        let mut row = proposal(ProposalAction::Widen { before: authority(), after: authority() });
        row.key = ProposalKey { proposer: maker, number: u64::MAX };
        row.proposer = maker;
        let key = ObjectKey::Proposal(row.key);
        let (mut d, id) = loaded(key, ReadValue::Proposal(Box::new(row)));
        let mut bindings = JigBindings::new(64, CLIENT.text);
        let tree = render(&d, id, words, &mut bindings);
        assert!(contains(&tree, label));
        let expected = skein_ui::Key { owner: crate::Owner::Jig.opaque(), kind, a: u64::MAX, b: u64::MAX };
        assert!(tree.nodes().iter().any(|node| node.role == Role::Item && node.key == Some(expected)));
        assert_eq!(crate::pages::object_key(key), expected);
        shown.push(expected);
        let action = press(&tree, b"Accept", &mut bindings);
        drive(&mut d, Event::Act { action });
        let tree = dialog(&d, words, &mut bindings);
        assert!(contains(&tree, label));
        let action = press(&tree, b"Confirm", &mut bindings);
        let sent = drive(&mut d, Event::Act { action });
        assert_eq!(sent.len(), 1);
        let Request::Client { asked: jig_client::Event::Send { request: jig_client::Ask::Jig(ask), .. } } = &sent[0]
        else {
            panic!("one typed proposal request")
        };
        assert_eq!(
            ask.as_ref(),
            &Ask::Decide {
                project: 4,
                on: DecideOn::Proposal {
                    proposal: ProposalKey { proposer: maker, number: u64::MAX },
                    decision: Decision::Accepted,
                    running: None
                }
            }
        );
    }
    assert_ne!(shown[0], shown[1]);
}

#[test]
fn proposed_outcomes_and_effect_parts_keep_the_supplied_maker_kind_and_full_numbers() {
    let words = words();
    for (maker, outcome, part) in [
        (
            Proposer::Task(u64::MAX),
            b"Proposed by task 18446744073709551615, proposal 18446744073709551615".as_slice(),
            b"Proposal by task 18446744073709551615, number 18446744073709551615".as_slice(),
        ),
        (
            Proposer::Party(u64::MAX),
            b"Proposed by party 18446744073709551615, proposal 18446744073709551615".as_slice(),
            b"Proposal by party 18446744073709551615, number 18446744073709551615".as_slice(),
        ),
    ] {
        let key = ProposalKey { proposer: maker, number: u64::MAX };
        let mut b = Builder::new(&limits());
        b.open(Role::Page);
        words.outcome(&mut b, &Outcome::Proposed { proposal: key });
        words.refusal(
            &mut b,
            &Refusal::Moved {
                now: Revision { value: u64::MAX },
                what: Moved::Hold {
                    reason: Some(CurrentHold::Document(Box::new(HoldReason::EffectFailed {
                        part: Part::Effect { connector: u16::MAX, proposal: key },
                    }))),
                },
            },
            CLIENT.text,
        );
        b.close();
        let tree = b.finish();
        assert!(contains(&tree, outcome));
        assert!(contains(&tree, part));
        let (by_party, by_task) = match maker {
            Proposer::Task(_) => (false, true),
            Proposer::Party(_) => (true, false),
        };
        assert_eq!(contains(&tree, b"by party"), by_party);
        assert_eq!(contains(&tree, b"by task"), by_task);
    }
}

#[test]
#[expect(clippy::match_like_matches_macro, reason = "scenario matching stays explicit; matches is disallowed")]
fn a_cut_note_shows_its_reference_loss_and_blocks_correction_but_keeps_exact_deletion() {
    let words = words();
    let mut bindings = JigBindings::new(512, CLIENT.text);
    for act in [ActKind::EditNote, ActKind::DeleteNote] {
        let note = Note {
            number: 77,
            revision: Revision { value: 123 },
            scope: NoteScope::Task { project: 1, task: 900 },
            description: DocumentText::complete(text(b"note title")),
            body: DocumentText::complete(text(b"note body")),
            writer: NoteAuthor::Party { party: 1 },
            tasks: list([91, u64::MAX]),
            changed: Some(Wall::EPOCH),
            acts: offered(act),
            cut: true,
        };
        let (mut domain, id) = note_loaded(note);
        drive(&mut domain, Event::Act { action: Action::Intend { object: id.token(), act } });
        let tree = dialog(&domain, words, &mut bindings);
        let outgoing = drive(&mut domain, Event::Act { action: Action::Confirm });
        if act == ActKind::EditNote {
            assert!(contains(&tree, b"This note has more references"));
            assert!(tree.nodes().iter().all(|node| node.value.is_none()));
            assert!(domain.confirmation().unwrap().missing.is_some());
            assert!(outgoing.iter().all(|request| match request {
                Request::Client { asked: jig_client::Event::Send { .. } } => false,
                _ => true,
            }));
        } else {
            assert!(outgoing.iter().any(|request| match request {
                Request::Client { asked: jig_client::Event::Send { request: jig_client::Ask::Jig(ask), .. } } =>
                    match &**ask {
                        Ask::DeleteNote {
                            note: 77,
                            scope: NoteScope::Task { project: 1, task: 900 },
                            revision: Revision { value: 123 },
                            ..
                        } => true,
                        _ => false,
                    },
                _ => false,
            }));
        }
    }
}
#[path = "instruction_facts.rs"]
mod instruction_facts;

#[test]
fn result_documents_keep_each_words_loss_fact_and_the_exact_change_kind_and_owner_part() {
    let (domain, _) = start();
    let words = words();
    let part = Part::Result { connector: 2, task: 8 };
    let raw = OwnerPart {
        connector: u16::MAX,
        kind: OwnerPartKind::Resource,
        key: OwnerPartKey::Path(list([b"a/b".as_slice().into(), b"c\xff".as_slice().into()])),
    };
    for cut in [false, true] {
        let explanation = DocumentText { bytes: text(b"supplied words"), cut };
        let values = [
            ResultValue::Report(explanation.clone()),
            ResultValue::Words(explanation.clone()),
            ResultValue::Failure(explanation.clone()),
            ResultValue::Verdict {
                verdict: u32::MAX,
                words: explanation.clone(),
                fields: list([]),
                tasks: list([]),
                proposal: None,
            },
            ResultValue::Connector { summary: explanation.clone(), part: part.clone() },
            ResultValue::Choice { option: u32::MAX, by: super::cards::party(), reason: explanation.clone() },
            ResultValue::Approved { state: Box::new([0, 255, 1]), words: Some(explanation.clone()) },
            ResultValue::Change { kind: u16::MAX, part: raw.clone(), words: explanation },
        ];
        for value in values {
            let mut builder = Builder::new(&crate::limits::of_words(&CLIENT, words).unwrap());
            builder.open(Role::Page);
            let mut bindings = JigBindings::new(64, CLIENT.text);
            cards::result::value_nodes(&mut builder, &domain.view(), &value, words, &mut bindings);
            builder.close();
            let tree = builder.finish();
            assert!(contains(&tree, b"supplied words"));
            assert_eq!(contains(&tree, b"Only part of this text was supplied"), cut);
            if let ResultValue::Change { .. } = value {
                assert!(contains(&tree, b"Change result kind 65535"));
                assert!(contains(&tree, b"a/b"));
                assert!(contains(&tree, b"\\x63\\xFF"));
                let slot = tree.nodes().iter().find_map(|node| node.slot).unwrap();
                assert_eq!(bindings.owner_part(slot.token), Some(&raw));
                assert_eq!(bindings.part(slot.token), None);
            }
        }
    }
}

#[test]
fn result_cards_distinguish_an_absent_completion_wall_from_epoch_and_render_supplied_metadata() {
    let (domain, _) = start();
    let words = words();
    for at in [None, Some(Wall::EPOCH)] {
        let mut value = result();
        value.at = at;
        value.details = Some(Box::new(ResultDetails {
            contract: TaskContractDocument::Report { words: u32::MAX },
            requester: OwnerRequester::Deployment { project: u32::MAX },
            executor: OwnerExecutor::Procedure { connector: u16::MAX, code: u32::MAX },
            chosen_by: None,
            delegates: list([
                OwnerTaskLink::Retained { chip: chip(), at: None },
                OwnerTaskLink::Stored { number: u64::MAX, status: None },
            ]),
            followups: Some(list([])),
            proposal: None,
            cut: false,
        }));
        let envelope = crate::limits::of_words(&CLIENT, words).unwrap();
        let mut builder = Builder::new(&envelope);
        builder.open(Role::Page);
        let mut bindings = JigBindings::new(envelope.bindings, CLIENT.text);
        cards::result::facts(&mut builder, &domain.view(), &value, words, &mut bindings);
        builder.close();
        let tree = builder.finish();
        assert_eq!(contains(&tree, b"Completion time not supplied"), at.is_none());
        assert!(contains(&tree, b"whole report"));
        assert!(contains(&tree, b"4294967295"));
        assert!(contains(&tree, b"18446744073709551615"));
    }
}

#[test]
fn exact_owner_verdict_renders_payload_and_stored_followups_without_inventing_chip_rows() {
    let (domain, _) = start();
    let words = words();
    for cut in [false, true] {
        let mut value = result();
        value.value = Some(ResultValue::OwnerVerdict {
            verdict: u32::MAX,
            words: DocumentText { bytes: text(b"actual verdict words"), cut },
            fields: list([ResultField { name: Box::from(*b"opaque field"), value: Box::from(*b"supplied value") }]),
        });
        value.details = Some(Box::new(ResultDetails {
            contract: TaskContractDocument::Verdict { choices: list([]), cut: true },
            requester: OwnerRequester::Person(1),
            executor: OwnerExecutor::Person(PersonAddress::Role(u32::MAX)),
            chosen_by: None,
            delegates: list([]),
            followups: Some(list([OwnerTaskLink::Stored { number: u64::MAX, status: None }])),
            proposal: Some(ProposalKey { proposer: Proposer::Party(u64::MAX), number: u64::MAX }),
            cut: true,
        }));
        let before = value.clone();
        let envelope = crate::limits::of_words(&CLIENT, words).unwrap();
        let mut builder = Builder::new(&envelope);
        builder.open(Role::Page);
        let mut bindings = JigBindings::new(envelope.bindings, CLIENT.text);
        cards::result::facts(&mut builder, &domain.view(), &value, words, &mut bindings);
        builder.close();
        let tree = builder.finish();
        assert!(contains(&tree, b"Verdict 4294967295"));
        assert!(contains(&tree, b"actual verdict words"));
        assert!(contains(&tree, b"opaque field"));
        assert!(contains(&tree, b"supplied value"));
        assert!(contains(&tree, b"18446744073709551615"));
        assert!(contains(&tree, b"Stored task"));
        assert_eq!(contains(&tree, b"Only part of this text was supplied"), cut);
        assert_eq!(value, before);
    }
}

#[path = "note_facts.rs"]
mod note_facts;

#[path = "task_links.rs"]
mod task_links;

#[path = "task_wake.rs"]
mod task_wake;

#[path = "task_contract.rs"]
mod task_contract;

#[path = "task_authority.rs"]
mod task_authority;

#[path = "task_budget.rs"]
mod task_budget;

mod amendment_reason;
#[path = "task_ending.rs"]
mod task_ending;

#[path = "task_resources.rs"]
mod task_resources;

#[path = "task_hold.rs"]
mod task_hold;

#[path = "task_context.rs"]
mod task_context;

#[path = "owner_escalation.rs"]
mod owner_escalation;

#[test]
fn connector_effect_slots_keep_raw_numbers_separate_from_jig_proposal_makers() {
    let words = words();
    let connector = u16::MAX;
    for proposal in [0, u64::MAX] {
        let native = Part::OwnerEffect { connector, proposal };
        let mut bindings = JigBindings::new(8, CLIENT.text);
        let mut builder = Builder::new(&limits());
        builder.open(Role::Page);
        cards::common::part(&mut builder, &native, words, &mut bindings);
        builder.close();
        let tree = builder.finish();
        let slot = tree.nodes().iter().find_map(|node| node.slot).unwrap();
        assert_eq!(slot.owner, (crate::Owner::Application { number: connector }).opaque());
        assert_eq!(bindings.part(slot.token), Some(&native));
        for proposer in [Proposer::Task(proposal), Proposer::Party(proposal)] {
            let maker_effect = Part::Effect { connector, proposal: ProposalKey { proposer, number: proposal } };
            let mut builder = Builder::new(&limits());
            builder.open(Role::Page);
            cards::common::part(&mut builder, &maker_effect, words, &mut bindings);
            builder.close();
            let tree = builder.finish();
            let old = tree.nodes().iter().find_map(|node| node.slot).unwrap();
            assert_eq!(old.owner, slot.owner);
            assert_ne!(old.token, slot.token);
            assert_eq!(bindings.part(old.token), Some(&maker_effect));
            assert_eq!(bindings.part(slot.token), Some(&native));
        }
        bindings.begin();
        assert_eq!(bindings.part(slot.token), None);
    }
}

#[test]
fn moved_effect_facts_say_the_supplied_connector_proposal_without_inventing_a_maker() {
    let words = words();
    for proposal in [0, u64::MAX] {
        let mut builder = Builder::new(&limits());
        builder.open(Role::Page);
        words.refusal(
            &mut builder,
            &Refusal::Moved {
                now: Revision { value: u64::MAX },
                what: Moved::Hold {
                    reason: Some(CurrentHold::Document(Box::new(HoldReason::EffectFailed {
                        part: Part::OwnerEffect { connector: u16::MAX, proposal },
                    }))),
                },
            },
            CLIENT.text,
        );
        builder.close();
        let tree = builder.finish();
        let expected =
            words.say(crate::phrases::jig::JigPhrase::PartOwnerProposal, &[skein_ui::phrases::Hole::Number(proposal)]);
        assert!(contains(&tree, b"Connector 65535"));
        assert!(contains(&tree, &expected));
        assert!(!contains(&tree, b"by task"));
        assert!(!contains(&tree, b"by party"));
        assert!(tree.nodes().iter().all(|node| node.slot.is_none() && node.binding.is_none()));
    }
}

const UTF8_TEXTS: [&[u8]; 1] = [b"\0~\x01"];

use jig_client::contract::DecideOn;

mod effect_fixtures;
use effect_fixtures::offered_acts;
