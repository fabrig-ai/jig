//! Full-list/full-text envelopes exercise public acquisition and every renderer.
use super::*;
use crate::Owner;
use crate::tests::fixtures::{Fixture, ReadPage, Watched};
use skein_ui::Limits;
use skein_ui::SlotRef;

use crate::address::Address;
use skein_lib::Token;

fn full_text() -> Text {
    // Maximum bytes, including the deepest allowed Markdown list path.
    let mut bytes = b"- a\n  - b\n    - c\n".to_vec();
    bytes.resize(usize::try_from(CLIENT.text).unwrap(), b'x');
    text(&bytes)
}
fn full_name() -> Text {
    text(UTF8_TEXTS[0])
}
fn full_owner_authority() -> OwnerAuthorityDocument {
    let pattern = full_pattern();
    let grant = OwnerGrant { connector: 2, kind: u16::MAX, pattern: pattern.clone() };
    let scope = OwnerResourceScope { connector: 2, pattern };
    OwnerAuthorityDocument {
        authority: OwnerAuthority {
            models: None,
            tools: u64::MAX,
            grants: list([grant.clone(), grant]),
            delegation: OwnerDelegation {
                kinds: list([OwnerAuthorityExecutor::Charter(u32::MAX), OwnerAuthorityExecutor::Role(u32::MAX)]),
                tasks: u32::MAX,
                depth: u32::MAX,
            },
            spend: u64::MAX,
            deadline: Some(Wall::from_nanos(u64::MAX)),
            notes: u8::MAX,
            note_resources: list([scope.clone(), scope]),
        },
        cut: true,
    }
}
fn full_authority() -> Authority {
    let mut value = authority();
    let grant = Grant { kind: 2, connector: 2, pattern: full_pattern() };
    value.grants = list([grant.clone(), grant]);
    value.task_kinds = list([Executor::Party { party: 1 }, Executor::Agent { charter: 2 }]);
    value.tools = list([1, 2]);
    let scope = NoteScope::Resource { project: 4, connector: 2, pattern: full_pattern() };
    value.note_scopes = list([scope.clone(), scope]);
    value
}
fn full_wake() -> OwnerWakePolicy {
    OwnerWakePolicy {
        words: WakeRule::Batch { count: u32::MAX, age: Duration::from_nanos(u64::MAX) },
        notices: WakeRule::Immediate,
        news: WakeRule::Batch { count: u32::MAX, age: Duration::from_nanos(u64::MAX) },
        results: ResultsWake::LastOrFailure,
        questions: true,
        answers: true,
        timers: true,
    }
}
fn full_subscriptions() -> Subscriptions {
    Subscriptions {
        entries: list([
            Subscription { number: u64::MAX, kind: SubscriptionKind::Topic { connector: u16::MAX, topic: u64::MAX } },
            Subscription {
                number: 2,
                kind: SubscriptionKind::Timer {
                    at: Wall::from_nanos(u64::MAX),
                    period: Some(Duration::from_nanos(u64::MAX)),
                },
            },
        ]),
    }
}
fn full_chip(task: u64) -> Chip {
    Chip {
        task,
        title: DocumentText::complete(full_name()),
        executor: Executor::Party { party: 1 },
        phase: Phase::Waiting,
        details: Box::new(ChipPhase::Waiting {
            dependencies: list([9, 10]),
            hold: Some(Part::Resource { connector: 2, path: full_path() }),
        }),
    }
}
fn full_trail() -> List<Passed> {
    let row = Passed {
        by: Actor::Party(1),
        at: Wall::from_nanos(u64::MAX),
        reason: full_name(),
        lacking: Some(Lacking::TaskKind { executor: Some(Executor::Party { party: 1 }) }),
    };
    list([row.clone(), row])
}
fn full_amendment() -> Amendment {
    Amendment {
        llm: None,
        addressed: Some(AddressChange { revision: Revision { value: 12 }, address: PersonAddress::Role(2) }),
        spec: Some(specification(full_text()).whole().unwrap()),
        instructions: Some(full_text()),
        wake: Some(full_wake()),
        dependencies: Some(list([9, 10])),
        authority: Some(full_owner_authority().authority),
        tracked: None,
    }
}
fn full_proposal(action: ProposalAction) -> Proposal {
    let mut value = proposal(action);
    value.waiting = Recipient::Roles { project: 4, roles: list([7, 2]) };
    value.why = full_text();
    value.needs = full_authority();
    value.trail = full_trail();
    value.ending = Some(Ending::Decided {
        by: 1,
        party: Some(Party { number: 1, kind: PartyKind::Person, name: full_name() }),
        how: Some(Decision::Rejected { reason: full_name() }),
        at: Wall::from_nanos(u64::MAX),
    });
    value
}
fn full_batch() -> ProposalAction {
    let mut row = super::super::proposed_task::full_template(1);
    row.spec.specification.words.bytes = full_text();
    row.executor = OwnerExecutor::Person(PersonAddress::Party(1));
    row.dependencies = list([1, 2]);
    ProposalAction::Batch { tasks: list([row.clone(), row]), own: true }
}
fn full_value() -> ResultValue {
    ResultValue::Verdict {
        verdict: 1,
        words: DocumentText::complete(full_text()),
        fields: list([
            ResultField { name: full_text().bytes().into(), value: full_text().bytes().into() },
            ResultField { name: full_text().bytes().into(), value: full_text().bytes().into() },
        ]),
        tasks: list([full_chip(11), full_chip(12)]),
        proposal: Some(ProposalKey { proposer: Proposer::Task(8), number: 1 }),
    }
}
fn full_result() -> Result {
    let mut value = result();
    value.task = full_chip(8);
    value.ending = Ending::Cancelled { by: Actor::Party(1), reason: Some(full_name()), at: None };
    value.value = Some(full_value());
    let choice = VerdictChoice {
        code: u32::MAX,
        words: u32::MAX,
        followups: u32::MAX,
        fields: list([
            FieldRule { name: full_text().bytes().into(), max: u32::MAX },
            FieldRule { name: full_text().bytes().into(), max: u32::MAX },
        ]),
    };
    value.details = Some(Box::new(ResultDetails {
        contract: TaskContractDocument::Verdict { choices: list([choice.clone(), choice]), cut: true },
        requester: OwnerRequester::Deployment { project: u32::MAX },
        executor: OwnerExecutor::Procedure { connector: u16::MAX, code: u32::MAX },
        chosen_by: Some(u64::MAX),
        delegates: list([
            OwnerTaskLink::Live(full_chip(13)),
            OwnerTaskLink::Retained { chip: full_chip(14), at: None },
        ]),
        followups: Some(list([
            OwnerTaskLink::Live(full_chip(15)),
            OwnerTaskLink::Retained { chip: full_chip(16), at: Some(Wall::from_nanos(u64::MAX)) },
        ])),
        proposal: None,
        cut: true,
    }));
    value
}
fn acquire(d: &mut Fixture, watch: jig_client::Handle, root: ObjectKey, key: ObjectKey, value: ReadValue) {
    let entry = InboxEntry {
        reply: None,
        lineage: list([]),
        goal: None,
        recipient: None,
        stall: None,
        object: root,
        project: 4,
        group: InboxGroup::Questions,
        position: 1,
        at: Wall::EPOCH,
    };
    let mut other = entry.clone();
    other.object = key;
    other.position = 2;
    let out = drive(
        d,
        Watched {
            watch,
            event: WatchEvent::Delivered(Delivery::Inbox(Box::new(Inbox {
                entries: list([entry, other]),
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
        .find_map(|request| match request {
            Request::Client { asked: jig_client::Event::Read { handle: read, .. } } => Some(*read),
            _ => None,
        })
        .unwrap();
    drive(d, ReadPage { read, result: ReadResult::Value(value) });
}
fn dependencies_and_proposal(d: &mut Fixture, watch: jig_client::Handle, root: ObjectKey) {
    let p = full_proposal(full_batch());
    acquire(d, watch, root, ObjectKey::Proposal(p.key), ReadValue::Proposal(Box::new(p)));
    let mut task = task_doc(&[ActKind::Cancel, ActKind::Amend]);
    task.chip = full_chip(9);
    acquire(d, watch, root, ObjectKey::Task(9), ReadValue::Task(Box::new(task)));
    assert_eq!(d.objects().len(), CLIENT.objects);
    assert!(d.objects().find(ObjectKey::Proposal(ProposalKey { proposer: Proposer::Task(8), number: 1 })).is_some());
}
fn card_tree(d: &Fixture, id: Id<Object>, words: &crate::words::Words, sized: &Limits, bound: u32) {
    let mut b = Builder::new(sized);
    let mut bindings = JigBindings::new(sized.bindings, CLIENT.text);
    b.open(Role::Page);
    assert!(cards::card(
        &mut b,
        &d.view(),
        id,
        &cards::Place::Inbox { lineage: list([full_chip(1), full_chip(2)]) },
        words,
        &mut bindings
    ));
    b.close();
    let tree = b.finish();
    assert!(tree.len() <= bound.checked_add(1).unwrap());
    super::super::cards_access::check(&tree);
    // The worst fixtures must actually exercise full payloads and slot fallbacks.
    assert!(contains(&tree, full_name().bytes()));
    assert!(tree.nodes().iter().any(|node| node.role == Role::Status));
}
fn full_cards(words: &crate::words::Words, sized: &Limits) {
    for action in [
        full_batch(),
        ProposalAction::Widen { before: full_authority(), after: full_authority() },
        ProposalAction::Amend { task: 8, before: full_text(), change: full_amendment() },
        ProposalAction::Effect { part: Part::Resource { connector: 2, path: full_path() } },
    ] {
        let person_value = full_proposal(action);
        let (d, id) = loaded(ObjectKey::Proposal(person_value.key), ReadValue::Proposal(Box::new(person_value)));
        card_tree(&d, id, words, sized, crate::proposal_nodes(&CLIENT).unwrap());
    }
    let mut question_value = question();
    question_value.words = DocumentText { bytes: full_text(), cut: true };
    question_value.task = full_chip(8);
    let (d, id) = loaded(ObjectKey::Question { task: 8, message: 1 }, ReadValue::Question(Box::new(question_value)));
    card_tree(&d, id, words, sized, crate::question_nodes(&CLIENT).unwrap());
    let options = list([
        OptionLine { number: 1, words: full_name(), words_max: 64, followups: 0, fields: list([]) },
        OptionLine { number: 2, words: full_name(), words_max: 64, followups: 0, fields: list([]) },
    ]);
    let mut person_value = person(PersonContract::Choice { options });
    person_value.task = full_chip(8);
    person_value.spec = specification(full_text());
    person_value.inputs = list([full_result(), full_result()]);
    person_value.taken.as_mut().unwrap().name = Some(full_name());
    person_value.deadline = Some(Wall::from_nanos(u64::MAX));
    let root = ObjectKey::PersonTask(8);
    let (mut d, id, watch) = loaded_with_watch(root, ReadValue::PersonTask(Box::new(person_value)));
    dependencies_and_proposal(&mut d, watch, root);
    drive(&mut d, Event::Act { action: Action::Intend { object: id.token(), act: ActKind::Answer } });
    card_tree(&d, id, words, sized, crate::person_task_nodes(&CLIENT).unwrap());
    let root = ObjectKey::Result(8);
    let (mut d, id, watch) = loaded_with_watch(root, ReadValue::Result(Box::new(full_result())));
    dependencies_and_proposal(&mut d, watch, root);
    card_tree(&d, id, words, sized, crate::result_nodes(&CLIENT).unwrap());
    let failure = AttemptFailure { attempt: 1, class: FailureClass::Permanent, reason: full_name(), at: Wall::EPOCH };
    let reason = HoldReason::Tries { failures: list([failure.clone(), failure]) };
    let mut escalation_value = escalation_doc(reason, &[ActKind::Release, ActKind::Amend]);
    escalation_value.task.title = DocumentText::complete(full_name());
    escalation_value.hold.result = Some(full_value());
    escalation_value.trail = full_trail();
    escalation_value.steering.spec = specification(full_text());
    escalation_value.steering.instructions = Some(DocumentText { bytes: full_text(), cut: true });
    escalation_value.steering.authority = full_owner_authority();
    escalation_value.steering.wake = full_wake();
    escalation_value.steering.dependencies = list([9, 10]);
    let root = ObjectKey::Escalation(8);
    let (mut d, id, watch) = loaded_with_watch(root, ReadValue::Escalation(Box::new(escalation_value)));
    dependencies_and_proposal(&mut d, watch, root);
    card_tree(&d, id, words, sized, crate::escalation_nodes(&CLIENT).unwrap());
    drive(&mut d, Event::Act { action: Action::AmendRelease { object: id.token() } });
    full_dialog(&d, words, sized);
    let note = Note {
        number: 1,
        revision: Revision { value: 2 },
        scope: NoteScope::Project(4),
        description: DocumentText::complete(full_name()),
        body: DocumentText::complete(full_text()),
        writer: NoteAuthor::Party { party: 1 },
        tasks: list([8, 9]),
        changed: Some(Wall::EPOCH),
        acts: offered(ActKind::EditNote),
        cut: false,
    };
    let (d, id) = note_loaded(note);
    card_tree(&d, id, words, sized, crate::card_nodes(&CLIENT).unwrap());
    let mut builder = Builder::new(sized);
    builder.open(Role::Page);
    cards::refusal(
        &mut builder,
        &Refusal::Ended {
            how: Ending::Decided {
                by: 1,
                party: Some(Party { number: 1, kind: PartyKind::Person, name: full_name() }),
                how: Some(Decision::Rejected { reason: full_name() }),
                at: Wall::from_nanos(u64::MAX),
            },
        },
        words,
        CLIENT.text,
    );
    builder.close();
    let tree = builder.finish();
    assert!(tree.len() <= crate::refusal_nodes(&CLIENT).unwrap().checked_add(1).unwrap());
    super::super::cards_access::check(&tree);
    let task = full_chat();
    let (mut d, id) = loaded(ObjectKey::Task(8), ReadValue::Task(Box::new(task)));
    card_tree(&d, id, words, sized, crate::card_nodes(&CLIENT).unwrap());
    drive(&mut d, Event::Act { action: Action::CloseChat { object: id.token() } });
    full_dialog(&d, words, sized);
}
fn full_dialog(d: &Fixture, words: &crate::words::Words, sized: &Limits) {
    let mut b = Builder::new(sized);
    let mut bindings = JigBindings::new(sized.bindings, CLIENT.text);
    b.open(Role::Page);
    crate::confirmation::confirmation(&mut b, &d.view(), words, &mut bindings);
    b.close();
    let tree = b.finish();
    assert!(tree.len() <= crate::confirmation_nodes(&CLIENT).unwrap().checked_add(1).unwrap());
    assert!(tree.nodes().iter().any(|node| node.role == Role::Dialog));
    super::super::cards_access::check(&tree);
}
fn full_frame_notices(words: &crate::words::Words, sized: &Limits) {
    use crate::page::navigation::{Destination, Navigation};
    let mut d = Fixture::new(&CLIENT);
    let mut entries = List::with_capacity(CLIENT.navigation_entries);
    for task in 0..CLIENT.navigation_entries {
        entries.push(Destination::Jig(Address::task(u64::from(task)))).unwrap();
    }
    d.configure_navigation(Navigation { home: Destination::Jig(Address::Inbox { project: None }), entries }).unwrap();
    let out = drive(
        &mut d,
        Event::Start {
            seed: 8,
            place: crate::address::Place::Jig(Address::Inbox { project: None }),
            records: list([]),
        },
    );
    let watch = out
        .iter()
        .find_map(|r| match r {
            Request::Client { asked: jig_client::Event::Watch { handle: watch, .. } } => Some(*watch),
            _ => None,
        })
        .unwrap();
    let mut f = document();
    f.party.name = DocumentText::complete(full_name());
    f.party.login = DocumentText::complete(full_name());
    f.projects.get_mut(0).unwrap().name = DocumentText::complete(full_name());
    let mut second = f.projects.get(0).unwrap().clone();
    second.project = 5;
    f.projects.push(second).unwrap();
    let out = drive(&mut d, Watched { watch, event: WatchEvent::Delivered(Delivery::Me(Box::new(f))) });
    let mut bindings = JigBindings::new(sized.bindings, CLIENT.text);
    let mut b = Builder::new(sized);
    b.open(Role::Page);
    crate::frame::banner(&mut b, &d.view(), words, &mut bindings);
    crate::frame::navigation(
        &mut b,
        &d.view(),
        SlotRef { owner: (Owner::Application { number: 1 }).opaque(), token: Token::new(1) },
        words,
        &mut bindings,
    );
    b.close();
    let tree = b.finish();
    assert!(
        tree.len()
            <= crate::banner_nodes(&CLIENT)
                .unwrap()
                .checked_add(crate::navigation_nodes(&CLIENT).unwrap())
                .unwrap()
                .checked_add(1)
                .unwrap()
    );
    assert_eq!(tree.nodes().iter().filter(|node| node.role == Role::Link).count(), 10);
    super::super::cards_access::check(&tree);
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
    full_replies_notices(&mut d, inbox, words, sized);
}
fn full_replies_notices(d: &mut Fixture, inbox: jig_client::Handle, words: &crate::words::Words, sized: &Limits) {
    let mut bindings = JigBindings::new(sized.bindings, CLIENT.text);
    let reply = TaskLine {
        chip: full_chip(8),
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
            numbers: OwnerNumbers { budget: 10, spent: 10, spent_below: 0, reserved: 0 },
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
        conversation: Some(ConversationRead { seen: 1, unread: Count { value: 2, limited: false } }),
    };
    let entry = InboxEntry {
        reply: Some(Box::new(reply)),
        lineage: list([full_chip(1), full_chip(2)]),
        goal: None,
        recipient: None,
        stall: None,
        object: ObjectKey::Task(8),
        project: 4,
        group: InboxGroup::Replies,
        position: 3,
        at: Wall::EPOCH,
    };
    drive(
        d,
        Watched {
            watch: inbox,
            event: WatchEvent::Delivered(Delivery::Inbox(Box::new(Inbox {
                entries: list([entry]),
                needs: document().needs,
                updates: Count { value: 0, limited: false },
                results_seen: 0,
                replies_seen: list([]),
                more: false,
            }))),
        },
    );
    card_tree(d, d.objects().find(ObjectKey::Task(8)).unwrap(), words, sized, crate::reply_nodes(&CLIENT).unwrap());
    for _ in 0..CLIENT.notices {
        crate::notices::kept::show(
            d,
            crate::notices::kept::Kind::Answer {
                object: ObjectKey::Task(8),
                answer: Answer::Done(Outcome::Made { what: Made::Tasks { tasks: list([9, 10]) } }),
            },
        );
    }
    let mut b = Builder::new(sized);
    b.open(Role::Page);
    crate::notices::notices(&mut b, &d.view(), words, &mut bindings);
    b.close();
    let tree = b.finish();
    assert!(tree.len() <= crate::notices_nodes(&CLIENT).unwrap().checked_add(1).unwrap());
    assert_eq!(
        tree.nodes().iter().filter(|node| node.role == Role::Status).count(),
        usize::try_from(CLIENT.notices).unwrap()
    );
    super::super::cards_access::check(&tree);
}
pub(super) fn scenario(words: &crate::words::Words) {
    let default = crate::limits::of(&CLIENT, &crate::page::Arrangement::none()).unwrap();
    let localized = crate::limits::of_words(&CLIENT, words).unwrap();
    assert!(default.text_per_node >= localized.text_per_node);

    let mut excessive = CLIENT;
    excessive.rows = u32::MAX;
    assert!(crate::card_nodes(&excessive).is_none());
    assert!(crate::limits::of(&excessive, &crate::page::Arrangement::none()).is_none());
    excessive = CLIENT;
    excessive.text = u32::MAX;
    assert!(crate::limits::of(&excessive, &crate::page::Arrangement::none()).is_none());
    full_cards(words, &localized);
    full_frame_notices(words, &localized);
}

fn specification(words: Text) -> SpecificationDocument {
    SpecificationDocument::complete(Specification {
        words,
        parameters: list([
            Parameter::Bytes { name: u32::MAX, value: full_text() },
            Parameter::Bytes { name: 0, value: full_text() },
        ]),
        inputs: list([u64::MAX, 0]),
    })
}

fn full_path() -> List<Box<[u8]>> {
    list([full_name().into_bytes(), full_name().into_bytes()])
}
fn full_pattern() -> OwnerPattern {
    OwnerPattern { segments: full_path(), last: OwnerPatternEnd::Exact(full_name().into_bytes()) }
}

const UTF8_TEXTS: [&[u8]; 1] = [&[b'n'; 128]];

fn full_chat() -> Task {
    let mut task = task_doc(&[ActKind::Cancel, ActKind::Amend]);
    task.chip = full_chip(8);
    task.flags.conversation = true;
    task.spec = specification(full_text());
    task.instructions = Some(DocumentText { bytes: full_text(), cut: true });
    task.wake = full_wake();
    task.subscriptions = full_subscriptions();

    task.authority = full_owner_authority();
    task.dependencies = list([OwnerTaskLink::Live(full_chip(9)), OwnerTaskLink::Live(full_chip(10))]);
    task.delegates = list([
        LiveDelegate { chip: full_chip(9), takeover_budget: 10 },
        LiveDelegate { chip: full_chip(10), takeover_budget: 20 },
    ]);
    task
}
