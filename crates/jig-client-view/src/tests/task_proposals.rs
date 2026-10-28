//! Shared proposals are rendered from supplied Task-page names, not invented calls.
#![expect(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::wildcard_enum_match_arm,
    reason = "ordinary proofs drive public events and inspect semantic trees"
)]
use super::{
    cards::{CLIENT, contains, drive, text, words},
    pages::deliver,
    work_pages::{entered, list, part},
};
use crate::JigBindings;
use crate::tests::fixtures::{Fixture, ReadPage};
use crate::{
    Event, Request,
    address::{Address, Place, TaskSection},
};
use jig_client::contract::*;
use skein_lib::Wall;
use skein_ui::{Builder, Role, Tree};
fn key(proposer: Proposer) -> ProposalKey {
    ProposalKey { proposer, number: 1 }
}
fn task(keys: &[ProposalKey], activity: u64) -> Task {
    let mut doc = super::cards_objects::task_doc(&[]);
    doc.chip.executor = Executor::Agent { charter: 2 };
    doc.chip.phase = Phase::Running;
    *doc.chip.details = ChipPhase::Running { attempt: 2, since: Wall::EPOCH, tool: None };
    doc.context = None;
    doc.hold = None;
    doc.proposals = list(keys.iter().copied());
    doc.activity = activity;
    doc
}
fn opened(keys: &[ProposalKey]) -> (Fixture, jig_client::Handle, Vec<Request>) {
    let (mut d, out) = entered(Address::Task { task: 8, section: TaskSection::Activity });
    let watch =
        out.iter()
            .find_map(|r| match r {
                Request::Client {
                    asked:
                        jig_client::Event::Watch {
                            handle: watch,
                            subject: jig_client::Watchable::Jig(Subject::Task { .. }),
                        },
                } => Some(*watch),
                _ => None,
            })
            .unwrap();
    let out = deliver(&mut d, watch, Delivery::Task(Box::new(task(keys, 11))));
    (d, watch, out)
}
fn proposal(key: ProposalKey) -> Proposal {
    Proposal {
        key,
        project: 4,
        proposer: key.proposer,
        why: text(b"whole shared reason"),
        action: ProposalAction::Effect { part: part() },
        needs: super::cards_objects::authority(),
        funding: Budget { total: 20, own: 0, ended: 0, reserved: 0, left: 20, funder: Actor::Party(1) },
        waiting: Recipient::Party(1),
        trail: list([]),
        since: Wall::EPOCH,
        acts: list([Offered { effect: None, act: ActKind::Accept, admission: Admission::Allowed }]),
        ending: None,
    }
}
fn token(out: &[Request], key: ProposalKey) -> jig_client::Handle {
    out.iter()
        .find_map(|r| match r {
            Request::Client { asked: jig_client::Event::Read { handle: read, read: jig_client::Readable::Jig(Read::Proposal { proposal }) } } if *proposal == key => Some(*read),
            _ => None,
        })
        .unwrap()
}
fn receive(d: &mut Fixture, out: &[Request], value: Proposal) {
    drive(d, ReadPage { read: token(out, value.key), result: ReadResult::Value(ReadValue::Proposal(Box::new(value))) });
}
fn build(d: &Fixture) -> Tree {
    let tree = build_tree(d);
    super::cards_access::check(&tree);
    tree
}
fn build_tree(d: &Fixture) -> Tree {
    let w = words();
    let nodes = crate::task_nodes(&CLIENT, 0)
        .unwrap()
        .checked_add(crate::confirmation_nodes(&CLIENT).unwrap())
        .unwrap()
        .checked_add(crate::notices_nodes(&CLIENT).unwrap())
        .unwrap()
        .checked_add(1)
        .unwrap();
    let mut limits = super::cards::limits();
    limits.nodes = nodes;
    limits.depth = 34;
    limits.bindings = nodes;
    limits.slots = nodes;
    limits.text_bytes = nodes.checked_mul(limits.text_per_node).unwrap();
    let mut b = Builder::new(&limits);
    let mut bindings = JigBindings::new(nodes, CLIENT.text);
    b.open(Role::Page);
    crate::pages::main(&mut b, &d.view(), w, &mut bindings);
    crate::pages::after_page(&mut b, &d.view(), w, &mut bindings);
    b.close();
    let tree = b.finish();
    assert!(tree.len() <= nodes);

    tree
}
fn cards(tree: &Tree, key: ProposalKey) -> usize {
    let wanted = crate::pages::object_key(ObjectKey::Proposal(key));
    tree.nodes().iter().filter(|n| n.key == Some(wanted)).count()
}
#[test]
fn a_task_activity_renders_whole_shared_proposals_without_any_decoded_call() {
    let first = key(Proposer::Task(8));
    let second = key(Proposer::Party(8));
    let (mut d, _, out) = opened(&[first, second]);
    for k in [first, second] {
        receive(&mut d, &out, proposal(k));
    }
    assert!(d.conversation().names(CLIENT.objects).is_empty());
    let tree = build(&d);
    assert_eq!(cards(&tree, first), 1);
    assert_eq!(cards(&tree, second), 1);
    assert!(contains(&tree, b"whole shared reason"));
    assert!(tree.nodes().iter().any(|node| node.role == Role::Button && node.name.as_deref() == Some(b"Accept")));
    assert_eq!(tree.nodes().iter().filter(|n| n.role == Role::Slot).count(), 2);
    let mut huge = CLIENT;
    huge.objects = u32::MAX;
    assert!(crate::task_nodes(&huge, 0).is_none());
    assert!(crate::task_nodes(&CLIENT, u32::MAX).is_none());
}
fn turn(objects: [ObjectKey; 1]) -> Turn {
    Turn {
        messages: list([Message {
            speaker: Speaker::Assistant,
            blocks: list([Block::Call(Call {
                number: 9,
                tool: text(b"Propose"),
                input: text(b"opaque input"),
                outcome: None,
                failed: false,
                objects: list(objects),
                parts: list([]),
            })]),
        }]),
        used: Usage { input: 0, output: 0, cached: 0, cache_written: 0 },
        spent: 0,
        read_through: None,
        cut: false,
    }
}
#[test]
fn task_proposals_share_exact_cards_with_committed_and_live_calls_without_conflating_makers() {
    for live in [false, true] {
        let first = key(Proposer::Task(8));
        let second = key(Proposer::Party(8));
        let (mut d, watch, out) = opened(&[first, second]);
        for k in [first, second] {
            receive(&mut d, &out, proposal(k));
        }
        let mut doc = task(&[first, second], 12);
        doc.context = super::cards_objects::task_doc(&[]).context;
        doc.context.as_mut().unwrap().runtime = Runtime::Running { attempt: 2, since: Wall::EPOCH, tool: None };
        deliver(&mut d, watch, Delivery::Task(Box::new(doc)));
        if live {
            let run = out
                .iter()
                .find_map(|r| match r {
                    Request::Client {
                        asked:
                            jig_client::Event::Watch {
                                handle: watch,
                                subject: jig_client::Watchable::Jig(Subject::Run { .. }),
                            },
                    } => Some(*watch),
                    _ => None,
                })
                .unwrap();
            let value = turn([ObjectKey::Proposal(first)]);
            let Block::Call(call) = value.messages.get(0).unwrap().blocks.get(0).unwrap() else {
                panic!("supplied call");
            };
            deliver(
                &mut d,
                run,
                Delivery::Run(Box::new(RunText {
                    writing: None,
                    attempt: 2,
                    began: None,
                    turn: ProvisionalTurnName { retry: 0, attempt: 2, number: 1 },
                    from: 0,
                    through: 0,
                    text: text(b""),
                    cut: false,
                    call: Some(LiveCall::Decoded(call.clone())),
                })),
            );
        } else {
            let read =
                out.iter()
                    .find_map(|r| match r {
                        Request::Client {
                            asked:
                                jig_client::Event::Read {
                                    handle: read,
                                    read: jig_client::Readable::Jig(Read::Activity { .. }),
                                },
                        } => Some(*read),
                        _ => None,
                    })
                    .unwrap();
            drive(
                &mut d,
                ReadPage {
                    read,
                    result: ReadResult::Value(ReadValue::Activity(Box::new(ActivityPage {
                        cut: false,
                        rows: list([ActivityLine {
                            position: 11,
                            at: Wall::EPOCH,
                            event: Activity::Turn {
                                name: TurnName { attempt: 2, number: 1 },
                                turn: TurnRead::Decoded(turn([ObjectKey::Proposal(first)])),
                            },
                        }]),
                        before: None,
                        after: None,
                    }))),
                },
            );
        }
        let tree = build(&d);
        assert_eq!(cards(&tree, first), 1);
        assert_eq!(cards(&tree, second), 1);
    }
}
#[test]
fn a_departing_task_proposal_shows_its_supplied_terminal_decision_until_page_departure() {
    let k = key(Proposer::Task(8));
    let (mut d, watch, out) = opened(&[k]);
    receive(&mut d, &out, proposal(k));
    let out = deliver(&mut d, watch, Delivery::Task(Box::new(task(&[], 12))));
    let mut ended = proposal(k);
    ended.ending = Some(Ending::Decided { by: 91, party: None, how: Some(Decision::Accepted), at: Wall::EPOCH });
    receive(&mut d, &out, ended);
    let tree = build(&d);
    assert_eq!(cards(&tree, k), 1);
    assert!(contains(&tree, b"Accepted"));
    assert!(contains(&tree, b"91"));
    assert!(!tree.nodes().iter().any(|n| n.role == Role::Button && n.name.as_deref() == Some(b"Accept")));
    drive(&mut d, Event::Went { place: Place::Elsewhere });
    assert!(d.task_page().proposals.is_empty());
}

#[test]
fn a_task_activity_renders_the_native_whole_pending_proposal_with_its_supplied_funding() {
    let k = key(Proposer::Task(8));
    let (mut d, _, out) = opened(&[k]);
    let native = OwnerProposal {
        key: k,
        project: 4,
        reason: DocumentText::complete(text(b"native whole shared reason")),
        action: OwnerProposalAction::Batch(list([])),
        as_holder: true,
        waiting: OwnerProposalWaiting { holder: OwnerHistoryHolder::Person(1), since: Wall::EPOCH },
        funding: list([OwnerFundingOffer {
            source: Funder::Pool { project: 4, person: 1, period: 3 },
            needed: 150,
            left: Some(250),
            after: Some(100),
        }]),
        acts: list([Offered { effect: None, act: ActKind::Accept, admission: Admission::Allowed }]),
    };
    drive(
        &mut d,
        ReadPage { read: token(&out, k), result: ReadResult::Value(ReadValue::OwnerProposal(Box::new(native))) },
    );
    let tree = build(&d);
    assert_eq!(cards(&tree, k), 1);
    assert!(contains(&tree, b"native whole shared reason"));
    assert!(contains(&tree, b"150"));
    assert!(contains(&tree, b"250"));
    assert!(contains(&tree, b"100"));
    assert!(tree.nodes().iter().any(|node| node.role == Role::Button && node.name.as_deref() == Some(b"Accept")));
}

fn leaving_plan(with_chat: bool) -> (Fixture, ProposalKey) {
    let k = key(Proposer::Task(8));
    let (mut d, opened) = super::work_pages::entered_arranged(
        Address::Task { task: 8, section: TaskSection::Plan },
        crate::page::Arrangement { task: list([TaskSection::Activity]), project: list([]) },
    );
    let watch =
        opened
            .iter()
            .find_map(|r| match r {
                Request::Client {
                    asked:
                        jig_client::Event::Watch {
                            handle: watch,
                            subject: jig_client::Watchable::Jig(Subject::Task { .. }),
                        },
                } => Some(*watch),
                _ => None,
            })
            .unwrap();
    let out = deliver(&mut d, watch, Delivery::Task(Box::new(task(&[k], 11))));
    receive(&mut d, &out, proposal(k));
    if with_chat {
        committed_proposal(&mut d, &out, k);
    }
    let id = d.objects().find(ObjectKey::Proposal(k)).unwrap();
    drive(&mut d, Event::Act { action: crate::action::Act::Intend { object: id.token(), act: ActKind::Accept } });
    drive(&mut d, Event::Act { action: crate::action::Act::Confirm });
    let tree =
        opened
            .iter()
            .chain(out.iter())
            .find_map(|r| match r {
                Request::Client {
                    asked:
                        jig_client::Event::Watch {
                            handle: watch,
                            subject: jig_client::Watchable::Jig(Subject::Tree { .. }),
                        },
                } => Some(*watch),
                _ => None,
            })
            .unwrap();
    deliver(
        &mut d,
        tree,
        Delivery::OwnerTree(Box::new(OwnerTree {
            root: super::work_pages::chip(8),
            rows: list([]),
            folded: list([]),
            held_below: 0,
            next: None,
        })),
    );
    (d, k)
}
fn committed_proposal(d: &mut Fixture, out: &[Request], k: ProposalKey) {
    let read = out
        .iter()
        .find_map(|r| match r {
            Request::Client {
                asked: jig_client::Event::Read { handle: read, read: jig_client::Readable::Jig(Read::Activity { .. }) },
            } => Some(*read),
            _ => None,
        })
        .unwrap();
    drive(
        d,
        ReadPage {
            read,
            result: ReadResult::Value(ReadValue::Activity(Box::new(ActivityPage {
                cut: false,
                rows: list([ActivityLine {
                    position: 11,
                    at: Wall::EPOCH,
                    event: Activity::Turn {
                        name: TurnName { attempt: 2, number: 1 },
                        turn: TurnRead::Decoded(turn([ObjectKey::Proposal(k)])),
                    },
                }]),
                before: None,
                after: None,
            }))),
        },
    );
}
#[test]
fn a_proposal_leaving_the_plan_is_owed_once() {
    let (d, k) = leaving_plan(false);
    let tree = build_tree(&d);
    assert_eq!(cards(&tree, k), 1);
    let owed = tree.nodes().iter().position(|n| n.region == Some(crate::regions::JigRegion::Owed.region())).unwrap();
    let card =
        tree.nodes().iter().position(|n| n.key == Some(crate::pages::object_key(ObjectKey::Proposal(k)))).unwrap();
    assert!(card > owed && card < owed + usize::try_from(tree.nodes()[owed].size).unwrap());
}
#[test]
fn a_proposal_its_chat_names_stays_in_the_chat_when_it_leaves_the_plan() {
    let (d, k) = leaving_plan(true);
    let id = d.objects().find(ObjectKey::Proposal(k)).unwrap();
    assert!(!d.owes(id));
    let tree = build_tree(&d);
    assert_eq!(cards(&tree, k), 1);
    for node in tree.nodes() {
        assert_ne!(node.region, Some(crate::regions::JigRegion::Owed.region()));
    }
}
