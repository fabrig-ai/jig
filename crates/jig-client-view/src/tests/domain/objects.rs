use super::spine::{LIMITS, delivered, drain, drive, env, frame, open_token, start};
use crate::tests::fixtures::refusal_answer;
use crate::tests::fixtures::{AnsweredByEngine, Fixture, ReadPage, Watched};
use crate::{
    Event, Request,
    intents::{Progress, send_card},
    objects::{self, Card, Left, Source},
};
use jig_client::contract::*;
use skein_lib::{List, Queue, Time, Wall};
pub(super) fn text(bytes: &[u8]) -> Text {
    Text::new(Box::from(bytes), 32).unwrap()
}
pub(super) fn chip(task: u64) -> Chip {
    Chip {
        task,
        title: DocumentText::complete(text(b"title")),
        executor: Executor::Party { party: 7 },
        phase: Phase::Held,
        details: Box::new(phase_details(Phase::Held)),
    }
}
pub(super) fn phase_details(phase: Phase) -> ChipPhase {
    match phase {
        Phase::Waiting => ChipPhase::Waiting { dependencies: List::with_capacity(0), hold: None },
        Phase::Idle => ChipPhase::Idle {
            wake: WakePolicy {
                words: true,
                results: true,
                topics: List::with_capacity(0),
                subscriptions: List::with_capacity(0),
            },
        },
        Phase::Due | Phase::Preparing | Phase::Claimed => ChipPhase::Starting,
        Phase::Running => ChipPhase::Running { attempt: 1, since: Wall::EPOCH, tool: None },
        Phase::BackingOff => ChipPhase::BackingOff {
            at: Wall::EPOCH,
            attempt: 1,
            tries: 3,
            class: FailureClass::Temporary,
            reason: text(b"temporary"),
        },
        Phase::Active => ChipPhase::Person { addressed: Recipient::Party(7), taken: None, since: Some(Wall::EPOCH) },
        Phase::Closing => ChipPhase::Closing(Closing {
            delegates: 0,
            connectors: Some(List::with_capacity(0)),
            stage: None,
            ending: None,
        }),
        Phase::Held => ChipPhase::Held {
            reason: HoldReason::Stopped { by: super::spine::party(7), at: Wall::EPOCH },
            since: Wall::EPOCH,
            revision: Revision { value: 3 },
            was: None,
        },
        Phase::Ended => {
            ChipPhase::Owner { phase: OwnerChipPhase::Ended, details: Box::new(super::chip_facts::details()) }
        }
        Phase::Done => ChipPhase::Ended { how: Ending::Done, at: Wall::EPOCH },
        Phase::Failed => ChipPhase::Ended { how: Ending::Failed { reason: Some(text(b"failure")) }, at: Wall::EPOCH },
        Phase::Cancelled => ChipPhase::Ended {
            how: Ending::Cancelled { by: Actor::Party(7), reason: Some(text(b"cancelled")), at: None },
            at: Wall::EPOCH,
        },
    }
}
pub(super) fn owner_authority() -> OwnerAuthorityDocument {
    OwnerAuthorityDocument {
        authority: OwnerAuthority {
            models: None,
            tools: 0,
            grants: List::with_capacity(0),
            delegation: OwnerDelegation { kinds: List::with_capacity(0), tasks: 1, depth: 1 },
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
        grants: List::with_capacity(0),
        task_kinds: List::with_capacity(0),
        depth: 1,
        tasks: 1,
        deadline: None,
        tools: List::with_capacity(0),
        note_scopes: List::with_capacity(0),
    }
}
pub(super) fn proposal() -> Proposal {
    Proposal {
        key: ProposalKey { proposer: Proposer::Task(8), number: 1 },
        project: 1,
        proposer: Proposer::Task(8),
        why: text(b"why"),
        action: ProposalAction::Widen { before: authority(), after: authority() },
        needs: authority(),
        funding: Budget { total: 10, own: 0, ended: 0, reserved: 0, left: 10, funder: Actor::Party(7) },
        waiting: Recipient::Party(7),
        trail: List::with_capacity(0),
        since: Wall::EPOCH,
        acts: List::with_capacity(0),
        ending: None,
    }
}
pub(super) fn question() -> Question {
    Question {
        task: chip(8),
        from: None,
        message: 1,
        words: DocumentText::complete(text(b"whole question")),
        since: Wall::EPOCH,
        acts: List::with_capacity(0),
        ending: None,
    }
}
pub(super) fn escalation() -> Escalation {
    Escalation {
        task: chip(8),
        hold: Hold {
            placed: None,
            revision: Revision { value: 3 },
            reason: HoldReason::Stopped { by: super::spine::party(7), at: Wall::EPOCH },
            before: Phase::Running,
            since: Wall::EPOCH,
            result: None,
            spend: List::with_capacity(0),
        },
        steering: Box::new(Steering {
            person_revision: None,
            taken_by: None,
            addressed: None,
            revision: Revision { value: 1 },
            spec: specification(text(b"held work")),
            instructions: Some(DocumentText::complete(text(b""))),
            wake: OwnerWakePolicy {
                words: WakeRule::Immediate,
                notices: WakeRule::Never,
                news: WakeRule::Never,
                results: ResultsWake::Each,
                questions: false,
                answers: false,
                timers: false,
            },
            dependencies: List::with_capacity(0),
            authority: owner_authority(),
            budget: Budget { total: 100, own: 100, ended: 0, reserved: 0, left: 0, funder: Actor::Party(1) },
        }),
        impact: Impact {
            tasks: PhaseCounts {
                ended: 0,
                waiting: 0,
                running: 0,
                held: 1,
                closing: 0,
                done: 0,
                failed: 0,
                cancelled: 0,
            },
            live_runs: 0,
            proposals: 0,
        },
        waiting: Recipient::Party(7),
        trail: List::with_capacity(0),
        acts: List::with_capacity(0),
        ending: None,
    }
}
pub(super) fn person() -> PersonTask {
    PersonTask {
        task: chip(8),
        revision: Revision { value: 4 },
        requester: Actor::Task(1),
        spec: specification(text(b"do it")),
        contract: PersonContract::Words,
        inputs: List::with_capacity(0),
        addressed: Recipient::Role { project: 1, role: 2 },
        taken: None,
        since: None,
        stall: None,
        deadline: None,
        acts: List::with_capacity(0),
        ending: None,
    }
}
pub(super) fn result() -> Result {
    Result {
        task: chip(8),
        ending: Ending::Done,
        value: Some(ResultValue::Report(DocumentText::complete(text(b"report")))),
        at: Some(Wall::EPOCH),
        acts: List::with_capacity(0),
        details: None,
    }
}
fn inbox(keys: &[ObjectKey]) -> Delivery {
    let mut entries = List::with_capacity(2);
    for key in keys {
        entries
            .push(InboxEntry {
                reply: None,
                lineage: List::with_capacity(0),
                goal: None,
                recipient: None,
                stall: None,
                object: *key,
                project: 1,
                group: InboxGroup::Proposals,
                position: 1,
                at: Wall::EPOCH,
            })
            .unwrap();
    }
    Delivery::Inbox(Box::new(Inbox {
        entries,
        needs: Needs { proposals: 0, questions: 0, escalations: 0, person_tasks: 0 },
        updates: Count { value: 0, limited: false },
        results_seen: 0,
        replies_seen: List::with_capacity(0),
        more: false,
    }))
}
pub(super) fn live() -> (Fixture, jig_client::Handle) {
    let mut domain = Fixture::new(&LIMITS);
    let f = start(&mut domain, List::with_capacity(0));
    let out = delivered(&mut domain, Time::ZERO, f, frame(7));
    let page = open_token(&out);
    super::pages::finish_initial_inbox_reads(&mut domain, &out);
    (domain, page)
}
pub(super) fn watched(domain: &mut Fixture, page: jig_client::Handle, keys: &[ObjectKey]) -> Box<[Request]> {
    drive(domain, Time::ZERO, Watched { watch: page, event: WatchEvent::Delivered(inbox(keys)) })
}
pub(super) fn read_token(rows: &[Request]) -> jig_client::Handle {
    for row in rows {
        if let Request::Client { asked: jig_client::Event::Read { handle: read, .. } } = row {
            return *read;
        }
    }
    unreachable!("an object read was emitted")
}

#[test]
#[expect(clippy::too_many_lines, reason = "the object fixture table names every unrelated read variant explicitly")]
fn documents_keep_whole_object_facts_revisions_and_reply_positions_within_the_working_set() {
    let (mut domain, page) = live();
    let cases = [
        (ObjectKey::Proposal(proposal().key), ReadValue::Proposal(Box::new(proposal()))),
        (ObjectKey::Escalation(8), ReadValue::Escalation(Box::new(escalation()))),
        (ObjectKey::Question { task: 8, message: 1 }, ReadValue::Question(Box::new(question()))),
        (ObjectKey::PersonTask(8), ReadValue::PersonTask(Box::new(person()))),
        (ObjectKey::Result(8), ReadValue::Result(Box::new(result()))),
    ];
    for (key, value) in cases {
        domain.objects.clear();
        domain.objects.reclaim();

        let requests = watched(&mut domain, page, &[key]);
        drive(
            &mut domain,
            Time::ZERO,
            ReadPage { read: read_token(&requests), result: ReadResult::Value(value.clone()) },
        );
        let id = domain.objects().find(key).unwrap();
        let object = domain.objects().get(id).unwrap();
        match value {
            ReadValue::Proposal(document) => assert_eq!(objects::proposal::facts(&object), Some(document.as_ref())),
            ReadValue::Escalation(document) => {
                assert_eq!(objects::escalation::facts(&object), Some(document.as_ref()));
                assert_eq!(objects::escalation::revision(&object), Some(Revision { value: 3 }));
            }
            ReadValue::Question(document) => assert_eq!(objects::question::facts(&object), Some(document.as_ref())),
            ReadValue::PersonTask(document) => {
                assert_eq!(objects::person_task::facts(&object), Some(document.as_ref()));
                assert_eq!(objects::person_task::revision(&object), Some(Revision { value: 4 }));
            }
            ReadValue::Result(document) => assert_eq!(objects::result::facts(&object), Some(document.as_ref())),
            ReadValue::OwnerProject(_)
            | ReadValue::Historical(_)
            | ReadValue::OwnerProposal(_)
            | ReadValue::OwnerPersonTask(_)
            | ReadValue::Tasks(_)
            | ReadValue::Me(_)
            | ReadValue::Task(_)
            | ReadValue::OwnerEscalation(_)
            | ReadValue::Ended(_)
            | ReadValue::OwnerEnded(_)
            | ReadValue::History(_)
            | ReadValue::Activity(_)
            | ReadValue::Turn(_)
            | ReadValue::Inbox(_)
            | ReadValue::OwnerInbox(_)
            | ReadValue::Spend(_)
            | ReadValue::OwnerSpend(_)
            | ReadValue::Project(_)
            | ReadValue::Notes(_)
            | ReadValue::Settings(_)
            | ReadValue::Setting(_)
            | ReadValue::Revision(_)
            | ReadValue::Revisions(_)
            | ReadValue::Deployment(_)
            | ReadValue::OwnerDeployment(_) => unreachable!(),
        }
    }
    let line = TaskLine {
        chip: chip(9),
        project: 1,
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
            funder: Funder::Pool { project: 1, person: 7, period: 3 },
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
    };
    let id = domain.objects.reply(Source::Window { number: 1 }, line).unwrap();
    let projected = domain.objects().get(id).unwrap();
    let reply = projected.reply.as_ref().unwrap();
    assert_eq!(reply.first_words.bytes(), b"title");
    assert_eq!(reply.first_unread, 6);
    assert!(domain.objects.name(ObjectKey::Task(10), Source::Window { number: 1 }).is_none());
}

#[test]
fn a_deciding_card_survives_a_drop_until_its_durable_answer_then_lingers() {
    let (mut domain, page) = live();
    let key = ObjectKey::Proposal(proposal().key);
    let reads = watched(&mut domain, page, &[key]);
    drive(
        &mut domain,
        Time::ZERO,
        ReadPage { read: read_token(&reads), result: ReadResult::Value(ReadValue::Proposal(Box::new(proposal()))) },
    );
    let mut out = Queue::with_capacity(crate::max_out(&LIMITS));
    let request = send_card(
        &mut domain.ui,
        key,
        ActKind::Accept,
        Box::new(Ask::Decide {
            project: 1,
            on: DecideOn::Proposal { proposal: proposal().key, decision: Decision::Accepted, running: None },
        }),
        &mut out,
        &domain.client,
    )
    .unwrap();
    assert_eq!(drain(out).len(), 1);
    let id = domain.objects().find(key).unwrap();
    assert!(watched(&mut domain, page, &[]).is_empty());
    assert_eq!(
        domain.objects().get(id).unwrap().card,
        Card::Deciding { request: Some(request), progress: Progress::InFlight }
    );
    assert!(domain.notices().ids().is_empty());
    let outcome = Outcome::DecidedFirst { by: super::spine::party(9), how: Decision::Accepted, at: Wall::EPOCH };
    drive(&mut domain, Time::from_nanos(20), AnsweredByEngine { request, answer: Answer::Done(outcome.clone()) });
    assert_eq!(
        domain.objects().get(id).unwrap().card,
        Card::Leaving { how: Left::Answered { outcome }, until: Time::from_nanos(5_000_000_020) }
    );
    assert!(domain.notices().ids().is_empty(), "a card still on this page owns its outcome");
    // Fire the unrelated heartbeat before the linger.
    let mut out = Queue::with_capacity(crate::max_out(&LIMITS));
    for _ in 0_u8..4 {
        crate::fire(&mut domain.ui, &env(Time::from_nanos(5_000_000_020)), &mut out, &domain.client);
    }
    assert!(domain.objects().find(key).is_none());
}

#[test]
fn a_drop_reads_how_the_card_ended_and_a_preexisting_read_can_supply_the_terminal() {
    for read_before_drop in [true, false] {
        let (mut domain, page) = live();
        let key = ObjectKey::Question { task: 8, message: 1 };
        let first = watched(&mut domain, page, &[key]);
        let initial = read_token(&first);
        if !read_before_drop {
            drive(
                &mut domain,
                Time::ZERO,
                ReadPage { read: initial, result: ReadResult::Value(ReadValue::Question(Box::new(question()))) },
            );
        }
        let dropped = watched(&mut domain, page, &[]);
        let terminal = if read_before_drop {
            assert!(dropped.is_empty());
            initial
        } else {
            read_token(&dropped)
        };
        let id = domain.objects().find(key).unwrap();
        let Card::Leaving { how, .. } = &domain.objects().get(id).unwrap().card else {
            panic!("terminal read keeps card leaving");
        };
        assert_eq!(*how, Left::Looking);
        let ending = Ending::Decided {
            by: frame(9).party.number,
            party: Some(super::spine::party(9)),
            how: Some(Decision::Accepted),
            at: Wall::EPOCH,
        };
        let mut ended = question();
        ended.ending = Some(ending.clone());
        drive(
            &mut domain,
            Time::from_nanos(30),
            ReadPage { read: terminal, result: ReadResult::Value(ReadValue::Question(Box::new(ended))) },
        );
        assert_eq!(
            domain.objects().get(id).unwrap().card,
            Card::Leaving { how: Left::Ended { how: ending }, until: Time::from_nanos(5_000_000_030) }
        );
        assert_ne!(domain.page(), &crate::page::Page::Missing);
        assert_eq!(domain.objects().get(id).unwrap().sources.len(), 0);
    }
}

fn specification(words: Text) -> SpecificationDocument {
    SpecificationDocument::complete(Specification {
        asks: false,
        words,
        parameters: List::with_capacity(0),
        inputs: List::with_capacity(0),
    })
}

#[test]
fn equal_number_task_and_party_proposals_keep_separate_requests_and_terminal_cards() {
    let (mut domain, _) = live();
    let mut pending = List::with_capacity(2);
    for maker in [Proposer::Task(u64::MAX), Proposer::Party(u64::MAX)] {
        let mut row = proposal();
        row.key = ProposalKey { proposer: maker, number: u64::MAX };
        row.proposer = maker;
        row.acts = List::with_capacity(1);
        row.acts.push(Offered { effect: None, act: ActKind::Accept, admission: Admission::Allowed }).unwrap();
        let key = ObjectKey::Proposal(row.key);
        let id = domain.objects.name(key, Source::Window { number: 4 }).unwrap();
        assert!(domain.objects.put(key, objects::Body::Proposal(Box::new(row.clone()))));
        domain.objects.get_mut(id).unwrap().project = Some(row.project);
        assert!(
            drive(
                &mut domain,
                Time::ZERO,
                Event::Act { action: crate::action::Act::Intend { object: id.token(), act: ActKind::Accept } }
            )
            .is_empty()
        );
        let sent = drive(&mut domain, Time::ZERO, Event::Act { action: crate::action::Act::Confirm });
        assert_eq!(sent.len(), 1);
        let Request::Client { asked: jig_client::Event::Send { key: request, request: jig_client::Ask::Jig(ask), .. } } =
            &sent[0]
        else {
            panic!("one keyed decision")
        };
        assert_eq!(
            ask.as_ref(),
            &Ask::Decide {
                project: row.project,
                on: DecideOn::Proposal { proposal: row.key, decision: Decision::Accepted, running: None }
            }
        );
        assert!(drive(&mut domain, Time::ZERO, Event::Act { action: crate::action::Act::Confirm }).is_empty());
        pending.push((key, id, *request)).unwrap();
    }
    assert_ne!(pending.get(0).unwrap().0, pending.get(1).unwrap().0);
    assert_ne!(pending.get(0).unwrap().1, pending.get(1).unwrap().1);
    assert_ne!(pending.get(0).unwrap().2, pending.get(1).unwrap().2);
    let outcome = Outcome::Decided { by: Actor::Party(7), how: Decision::Accepted, at: Wall::EPOCH };
    let answer = Answer::Done(outcome.clone());
    assert!(
        drive(&mut domain, Time::ZERO, AnsweredByEngine { request: pending.get(0).unwrap().2, answer: answer.clone() })
            .is_empty()
    );
    assert_eq!(
        domain.objects().get(pending.get(0).unwrap().1).unwrap().card,
        Card::Leaving { how: Left::Answered { outcome: outcome.clone() }, until: Time::from_nanos(5_000_000_000) }
    );
    assert_eq!(
        domain.objects().get(pending.get(1).unwrap().1).unwrap().card,
        Card::Deciding { request: Some(domain.intent(pending.get(1).unwrap().2)), progress: Progress::InFlight }
    );
    assert_eq!(domain.objects().get(pending.get(0).unwrap().1).unwrap().answer, Some(answer.clone()));
    assert!(
        drive(
            &mut domain,
            Time::ZERO,
            AnsweredByEngine { request: pending.get(0).unwrap().2, answer: refusal_answer(Refusal::KeyConflict) }
        )
        .is_empty()
    );
    assert_eq!(domain.objects().get(pending.get(0).unwrap().1).unwrap().answer, Some(answer.clone()));
    assert!(
        drive(&mut domain, Time::ZERO, AnsweredByEngine { request: pending.get(1).unwrap().2, answer: answer.clone() })
            .is_empty()
    );
    assert_eq!(domain.objects().get(pending.get(1).unwrap().1).unwrap().answer, Some(answer));
    assert_eq!(
        domain.objects().get(pending.get(1).unwrap().1).unwrap().card,
        Card::Leaving { how: Left::Answered { outcome }, until: Time::from_nanos(5_000_000_000) }
    );
    for (key, id, _) in &pending {
        assert_eq!(domain.objects().find(*key), Some(*id));
    }
}

#[test]
fn a_party_proposal_maker_does_not_supply_a_task_address() {
    for number in [0, u64::MAX] {
        assert_eq!(
            crate::page::inbox::address(ObjectKey::Proposal(ProposalKey {
                proposer: Proposer::Task(number),
                number: u64::MAX,
            })),
            Some(crate::address::Address::task(number))
        );
        assert_eq!(
            crate::page::inbox::address(ObjectKey::Proposal(ProposalKey {
                proposer: Proposer::Party(number),
                number: u64::MAX,
            })),
            None
        );
    }
}

use jig_client::contract::DecideOn;
