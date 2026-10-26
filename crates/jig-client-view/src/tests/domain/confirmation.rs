use super::{
    objects::{chip, escalation, live, owner_authority, person, proposal, text},
    spine::drive,
};
use crate::tests::fixtures::{Fixture, ReadPage};
use crate::{
    Event, Request,
    action::Act,
    acts,
    confirm::Missing,
    fields::Purpose,
    objects::{Body, Card, Object, Source},
};
use jig_client::contract::*;
use jig_client::contract::{AnswerOn, DecideOn};
use skein_lib::{Id, List, Time, Wall};
fn offered(act: ActKind, admission: Admission) -> List<Offered> {
    let mut rows = List::with_capacity(2);
    rows.push(Offered { effect: None, act, admission }).unwrap();
    if act == ActKind::Amend {
        rows.push(Offered { effect: None, act: ActKind::Release, admission: Admission::Allowed }).unwrap();
    }
    rows
}
pub(super) fn install(domain: &mut Fixture, body: Body) -> Id<Object> {
    let key = body.key();
    let id = domain.objects.name(key, Source::Window { number: 4 }).unwrap();
    assert!(domain.objects.put(key, body));
    domain.objects.get_mut(id).unwrap().project = Some(1);
    id
}
fn intend(domain: &mut Fixture, id: Id<Object>, act: ActKind) -> Box<[Request]> {
    drive(domain, Time::ZERO, Event::Act { action: Act::Intend { object: id.token(), act } })
}
pub(super) fn change() -> Amendment {
    Amendment {
        llm: None,
        addressed: None,
        spec: specification(text(b"amended")).whole(),
        wake: None,
        dependencies: None,
        authority: None,
        instructions: None,
        tracked: None,
    }
}
fn counts() -> PhaseCounts {
    PhaseCounts { ended: 0, waiting: 0, running: 0, held: 0, closing: 0, done: 0, failed: 0, cancelled: 0 }
}
pub(super) fn task() -> Task {
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
        chip: chip(8),
        goal: None,
        priority: None,
        lineage_chips: List::with_capacity(0),
        inputs: List::with_capacity(0),
        project: 1,
        requester: OwnerRequester::Person(7),
        lineage: List::with_capacity(0),
        spec: specification(text(b"spec")),
        spec_revision: Revision { value: 9 },
        person_revision: None,
        taken_by: None,
        instructions: Some(DocumentText::complete(text(b"instructions"))),
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
        dependencies: List::with_capacity(0),
        cut: false,
        references: List::with_capacity(0),
        resources: List::with_capacity(0),
        budget: OwnerBudget {
            numbers: OwnerNumbers { budget: 10, spent: 0, spent_below: 0, reserved: 0 },
            left: 10,
            funder: Funder::Pool { project: 1, person: 7, period: 3 },
            deadline: None,
        },
        authority: owner_authority(),
        hold: Some(TaskHold::Document(Box::new(escalation().hold))),
        proposals: List::with_capacity(0),
        activity: 0,
        changed: Wall::EPOCH,
        acts: List::with_capacity(0),
        result: None,
        delegates: List::with_capacity(0),

        context: Some(Box::new(TaskContext {
            escalation: None,
            parts: List::with_capacity(0),
            closing: None,
            spend: List::with_capacity(0),
            runtime: Runtime::Held { was: None },
            impact: Impact { tasks: counts(), live_runs: 0, proposals: 0 },
            writer_slots: List::with_capacity(0),
        })),
    }
}
fn sent(rows: &[Request]) -> &Ask {
    let Request::Client { asked: jig_client::Event::Send { request: jig_client::Ask::Jig(ask), .. } } = &rows[0] else {
        panic!("one request");
    };
    assert_eq!(rows.len(), 1);
    ask
}
fn edit(domain: &mut Fixture, field: Purpose, bytes: &[u8]) {
    assert!(drive(domain, Time::ZERO, Event::Act { action: Act::Edit { field, text: text(bytes) } }).is_empty());
}
pub(super) fn reread(domain: &mut Fixture, body: Body, what: Read) {
    let read = domain.request_read(Source::Window { number: 4 }, what).unwrap();
    let value = match body {
        Body::Escalation(document) => ReadValue::Escalation(document),
        Body::OwnerEscalation(document) => ReadValue::OwnerEscalation(document),
        Body::Task(document) => ReadValue::Task(document),
        Body::PersonTask(document) => ReadValue::PersonTask(document),
        Body::OwnerPersonTask(document) => ReadValue::OwnerPersonTask(document),
        Body::Proposal(_)
        | Body::OwnerProposal(_)
        | Body::Question(_)
        | Body::Result(_)
        | Body::Historical(_)
        | Body::Chip(_)
        | Body::Note { .. } => unreachable!(),
    };
    drive(domain, Time::ZERO, ReadPage { read, result: ReadResult::Value(value) });
}

#[test]
fn each_decision_sends_only_its_recalled_revision_and_double_confirmation_sends_once() {
    for case in 0_u8..5 {
        let (mut domain, _) = live();
        let (body, act) = match case {
            0 => {
                let mut row = escalation();
                row.acts = offered(ActKind::Release, Admission::Allowed);
                (Body::Escalation(Box::new(row)), ActKind::Release)
            }
            1 => {
                let mut row = task();
                row.acts = offered(ActKind::Amend, Admission::Allowed);
                (Body::Task(Box::new(row)), ActKind::Amend)
            }
            2 => {
                let mut row = person();
                row.acts = offered(ActKind::Answer, Admission::Allowed);
                (Body::PersonTask(Box::new(row)), ActKind::Answer)
            }
            3 => {
                let mut row = proposal();
                row.acts = offered(ActKind::Accept, Admission::Allowed);
                (Body::Proposal(Box::new(row)), ActKind::Accept)
            }
            4 => {
                let mut row = task();
                row.acts = offered(ActKind::Stop, Admission::Allowed);
                (Body::Task(Box::new(row)), ActKind::Stop)
            }
            _ => unreachable!(),
        };
        let id = install(&mut domain, body);
        assert!(intend(&mut domain, id, act).is_empty());
        assert_eq!(
            domain.objects().get(id).unwrap().card,
            Card::Deciding { request: None, progress: crate::intents::Progress::Confirming }
        );
        match case {
            1 => {
                drive(
                    &mut domain,
                    Time::ZERO,
                    Event::Act { action: Act::Edit { field: Purpose::Reason, text: text(b"") } },
                );
                drive(&mut domain, Time::ZERO, Event::Act { action: Act::Change { change: Box::new(change()) } });
            }
            2 => edit(&mut domain, Purpose::Answer, b"the answer"),
            0 | 3 | 4 => {}
            _ => unreachable!(),
        }
        let rows = drive(&mut domain, Time::ZERO, Event::Act { action: Act::Confirm });
        let expected = match case {
            0 => Ask::Release { project: 1, task: 8, revision: Revision { value: 3 }, again: None },
            1 => Ask::Amend {
                reason: text(b""),
                project: 1,
                task: 8,
                revision: Some(Revision { value: 9 }),
                change: Box::new(change()),
            },
            2 => Ask::Answer {
                project: 1,
                on: AnswerOn::Person {
                    task: 8,
                    revision: Revision { value: 4 },
                    answer: PersonAnswer::Words(text(b"the answer")),
                },
            },
            3 => Ask::Decide {
                project: 1,
                on: DecideOn::Proposal { proposal: proposal().key, decision: Decision::Accepted, running: None },
            },
            4 => Ask::Stop { project: 1, task: 8 },
            _ => unreachable!(),
        };
        assert_eq!(sent(&rows), &expected);
        assert!(drive(&mut domain, Time::ZERO, Event::Act { action: Act::Confirm }).is_empty());
        assert!(domain.confirmation().is_none());
        if case == 2 {
            assert!(domain.fields().get(Purpose::Answer).unwrap().text.bytes().is_empty());
            assert_eq!(domain.fields().get(Purpose::Answer).unwrap().writes, 1);
        }
    }
}

#[test]
fn note_corrections_keep_the_supplied_scope_revision_and_all_references() {
    let mut note = Note {
        number: 6,
        revision: Revision { value: 11 },
        scope: NoteScope::Task { project: 1, task: 900 },
        description: DocumentText::complete(text(b"before")),
        body: DocumentText::complete(text(b"body")),
        writer: NoteAuthor::Party { party: 7 },
        tasks: {
            let mut rows = List::with_capacity(2);
            rows.push(91).unwrap();
            rows.push(u64::MAX).unwrap();
            rows
        },
        changed: Some(Wall::EPOCH),
        acts: List::with_capacity(2),
        cut: false,
    };
    note.acts.push(Offered { effect: None, act: ActKind::EditNote, admission: Admission::Allowed }).unwrap();
    note.acts.push(Offered { effect: None, act: ActKind::DeleteNote, admission: Admission::Allowed }).unwrap();
    assert_eq!(
        acts::note_ask(1, &note, ActKind::EditNote, text(b"after"), text(b"new")),
        Some(Ask::EditNote {
            project: 1,
            note: 6,
            scope: NoteScope::Task { project: 1, task: 900 },
            revision: Revision { value: 11 },
            description: text(b"after"),
            body: text(b"new"),
            references: note.tasks.clone()
        })
    );
    assert_eq!(
        acts::note_ask(1, &note, ActKind::DeleteNote, text(b""), text(b"")),
        Some(Ask::DeleteNote {
            project: 1,
            note: 6,
            scope: NoteScope::Task { project: 1, task: 900 },
            revision: Revision { value: 11 }
        })
    );
    for act in [ActKind::EditNote, ActKind::DeleteNote] {
        let (mut domain, _) = live();
        let id = domain.objects.note(1, Source::Window { number: 4 }, note.clone()).unwrap();
        assert!(intend(&mut domain, id, act).is_empty());
        assert_eq!(domain.confirmation().unwrap().revision, Some(Revision { value: 11 }));
        let expected = if act == ActKind::EditNote {
            edit(&mut domain, Purpose::NoteDescription, b"after");
            edit(&mut domain, Purpose::NoteBody, b"new");
            Ask::EditNote {
                project: 1,
                note: 6,
                scope: NoteScope::Task { project: 1, task: 900 },
                revision: Revision { value: 11 },
                description: text(b"after"),
                body: text(b"new"),
                references: note.tasks.clone(),
            }
        } else {
            Ask::DeleteNote {
                project: 1,
                note: 6,
                scope: NoteScope::Task { project: 1, task: 900 },
                revision: Revision { value: 11 },
            }
        };
        assert_eq!(sent(&drive(&mut domain, Time::ZERO, Event::Act { action: Act::Confirm })), &expected);
        assert!(domain.confirmation().is_none());
        if act == ActKind::EditNote {
            assert!(domain.fields().get(Purpose::NoteDescription).unwrap().text.bytes().is_empty());
            assert!(domain.fields().get(Purpose::NoteBody).unwrap().text.bytes().is_empty());
        }
    }
}

#[test]
fn a_changed_revision_refuses_in_the_confirmation_while_spend_changes_only_its_words() {
    let (mut domain, _) = live();
    let mut row = escalation();
    row.acts = offered(ActKind::Release, Admission::Allowed);
    let id = install(&mut domain, Body::Escalation(Box::new(row.clone())));
    intend(&mut domain, id, ActKind::Release);
    row.hold.revision = Revision { value: 8 };
    reread(&mut domain, Body::Escalation(Box::new(row)), Read::Escalation { task: 8 });
    assert!(domain.confirmation().unwrap().changed);
    assert_eq!(domain.confirmation().unwrap().missing, Some(Missing::Changed { now: Some(Revision { value: 8 }) }));
    assert!(drive(&mut domain, Time::ZERO, Event::Act { action: Act::Confirm }).is_empty());
    drive(&mut domain, Time::ZERO, Event::Act { action: Act::Dismiss });
    domain.objects.clear();
    domain.objects.reclaim();

    let mut row = task();
    row.acts = offered(ActKind::Amend, Admission::Proposed { to: Recipient::Party(9) });
    let id = install(&mut domain, Body::Task(Box::new(row.clone())));
    intend(&mut domain, id, ActKind::Amend);
    drive(&mut domain, Time::ZERO, Event::Act { action: Act::Edit { field: Purpose::Reason, text: text(b"") } });
    drive(&mut domain, Time::ZERO, Event::Act { action: Act::Change { change: Box::new(change()) } });
    row.budget.numbers.spent = 5;
    reread(&mut domain, Body::Task(Box::new(row)), Read::Task { task: 8 });
    let open = domain.confirmation().unwrap();
    assert!(!open.changed);
    assert_eq!(open.proposed, Some(Recipient::Party(9)));
    assert!(open.missing.is_none());
    assert_eq!(
        sent(&drive(&mut domain, Time::ZERO, Event::Act { action: Act::Confirm })),
        &Ask::Propose {
            project: 1,
            action: ProposeAction::Amendment { task: 8, revision: Revision { value: 9 }, change: Box::new(change()) },
            reason: text(b""),
        }
    );
}

#[test]
fn only_engine_offers_open_and_new_confirmations_or_dismissals_clear_the_old_fields() {
    let (mut domain, _) = live();
    let mut row = proposal();
    row.acts = offered(ActKind::Reject, Admission::Allowed);
    let id = install(&mut domain, Body::Proposal(Box::new(row.clone())));
    assert!(intend(&mut domain, id, ActKind::Accept).is_empty());
    assert!(domain.confirmation().is_none());
    assert_eq!(domain.objects().get(id).unwrap().card, Card::Refused { refusal: Refusal::NotOffered });
    intend(&mut domain, id, ActKind::Reject);
    assert!(domain.confirmation().unwrap().dangerous);
    assert_eq!(domain.confirmation().unwrap().missing, Some(Missing::Words { field: Purpose::Reason }));
    edit(&mut domain, Purpose::Reason, b"reason");
    drive(&mut domain, Time::ZERO, Event::Act { action: Act::Dismiss });
    assert!(domain.fields().get(Purpose::Reason).unwrap().text.bytes().is_empty());
    assert_eq!(domain.fields().get(Purpose::Reason).unwrap().writes, 1);
    intend(&mut domain, id, ActKind::Reject);
    edit(&mut domain, Purpose::Reason, b"new reason");
    // A new act closes the old confirmation even when the new act is not offered.
    intend(&mut domain, id, ActKind::Accept);
    assert!(domain.confirmation().is_none());
    assert!(domain.fields().get(Purpose::Reason).unwrap().text.bytes().is_empty());
    assert_eq!(domain.fields().get(Purpose::Reason).unwrap().writes, 2);
}

#[test]
fn bounded_amendment_confirms_supplied_budget_and_grants_and_refreshes_a_moved_offer() {
    bounded_amendment_dimensions();
    for release in [false, true] {
        let act = ActKind::Amend;
        let (mut domain, _) = live();
        let mut row = task();
        let grant = bounded_grant();
        let mut grants = List::with_capacity(1);
        grants.push(grant.clone()).unwrap();
        row.acts = offered(
            act,
            Admission::Bounded {
                within: Bounds::Amend {
                    budget_left: 5,
                    implications: List::with_capacity(0),
                    grants: grants.clone(),
                    authority: Box::new({
                        let mut allowed = row.authority.clone();
                        allowed.authority.spend = 15;
                        allowed.authority.grants = grants.clone();
                        allowed
                    }),
                    ceiling: Box::new({
                        let mut maximum = row.authority.clone();
                        maximum.authority.spend = 100;
                        maximum.authority.grants = grants;
                        maximum
                    }),
                },
                beyond: Beyond::Proposed { to: Recipient::Party(9) },
            },
        );
        let id = install(&mut domain, Body::Task(Box::new(row.clone())));
        if release {
            intend_amend_release(&mut domain, id);
        } else {
            intend(&mut domain, id, act);
        }
        assert_eq!(domain.confirmation().unwrap().missing, Some(Missing::Change));
        let mut draft = change();
        let mut after = row.authority.whole().unwrap();
        after.spend = 15;
        after.grants = List::with_capacity(1);
        after.grants.push(grant).unwrap();
        draft.authority = Some(after);
        drive(&mut domain, Time::ZERO, Event::Act { action: Act::Edit { field: Purpose::Reason, text: text(b"") } });
        drive(&mut domain, Time::ZERO, Event::Act { action: Act::Change { change: Box::new(draft.clone()) } });
        assert!(domain.confirmation().unwrap().missing.is_none());
        assert_eq!(domain.confirmation().unwrap().proposed, None);
        draft.authority.as_mut().unwrap().spend = 16;
        drive(&mut domain, Time::ZERO, Event::Act { action: Act::Edit { field: Purpose::Reason, text: text(b"") } });
        drive(&mut domain, Time::ZERO, Event::Act { action: Act::Change { change: Box::new(draft.clone()) } });
        assert_eq!(domain.confirmation().unwrap().proposed, Some(Recipient::Party(9)));
        row.acts = offered(
            act,
            Admission::Bounded {
                within: Bounds::Amend {
                    budget_left: 5,
                    implications: List::with_capacity(0),
                    grants: List::with_capacity(0),
                    authority: Box::new(row.authority.clone()),
                    ceiling: Box::new({
                        let mut maximum = row.authority.clone();
                        maximum.authority.spend = 100;
                        maximum.authority.grants = draft.authority.as_ref().unwrap().grants.clone();
                        maximum
                    }),
                },
                beyond: Beyond::Refused(Refusal::Role { held: Some(HeldRole::Policy { role: 2 }) }),
            },
        );
        reread(&mut domain, Body::Task(Box::new(row.clone())), Read::Task { task: 8 });
        assert_eq!(
            domain.confirmation().unwrap().missing,
            Some(Missing::Refused { why: Refusal::Role { held: Some(HeldRole::Policy { role: 2 }) } })
        );
        assert!(drive(&mut domain, Time::ZERO, Event::Act { action: Act::Confirm }).is_empty());
        draft.authority = None;
        drive(&mut domain, Time::ZERO, Event::Act { action: Act::Edit { field: Purpose::Reason, text: text(b"") } });
        drive(&mut domain, Time::ZERO, Event::Act { action: Act::Change { change: Box::new(draft.clone()) } });
        assert_eq!(domain.confirmation().unwrap().proposed, None);
        assert_eq!(
            sent(&drive(&mut domain, Time::ZERO, Event::Act { action: Act::Confirm })),
            &crate::tests::fixtures::releasing(
                Ask::Amend {
                    reason: text(b""),
                    project: 1,
                    task: 8,
                    revision: Some(row.spec_revision),
                    change: Box::new(draft),
                },
                if release { Some(row.hold.as_ref().unwrap().revision()) } else { None }
            )
        );
    }
}

fn bounded_amendment_dimensions() {
    let before = owner_authority();
    let mut facts = before.authority.clone();
    facts.spend = 20;
    facts.delegation.depth = 3;
    facts.delegation.tasks = 5;
    facts.deadline = Some(Wall::from_nanos(100));
    facts.tools = 1_u64 << 7_u32;
    facts.delegation.kinds = List::with_capacity(1);
    facts.delegation.kinds.push(OwnerAuthorityExecutor::Role(4)).unwrap();
    facts.notes = 1;
    facts.note_resources = List::with_capacity(1);
    facts
        .note_resources
        .push(OwnerResourceScope {
            connector: 1,
            pattern: OwnerPattern {
                segments: List::with_capacity(0),
                last: OwnerPatternEnd::Exact(Box::from(*b"listed")),
            },
        })
        .unwrap();
    let mut maximum = facts.clone();
    maximum.delegation.tasks = 6;
    maximum.delegation.depth = 4;
    maximum.spend = 100;
    maximum.deadline = None;
    let offer = Admission::Bounded {
        within: Bounds::Amend {
            budget_left: 10,
            implications: List::with_capacity(0),
            grants: List::with_capacity(0),
            authority: Box::new(OwnerAuthorityDocument::complete(facts.clone())),
            ceiling: Box::new(OwnerAuthorityDocument::complete(maximum)),
        },
        beyond: Beyond::Proposed { to: Recipient::Party(9) },
    };
    let mut draft = change();
    draft.authority = Some(facts.clone());
    assert_eq!(admission::amendment(&offer, &draft, &before), Admission::Allowed);
    for dimension in 0_u8..7 {
        let mut outside = facts.clone();
        match dimension {
            0 => outside.spend = 21,
            1 => outside.delegation.depth = 4,
            2 => outside.delegation.tasks = 6,
            3 => outside.deadline = Some(Wall::from_nanos(101)),
            4 => outside.tools = 1_u64 << 8_u32,
            5 => *outside.delegation.kinds.get_mut(0).unwrap() = OwnerAuthorityExecutor::Role(5),
            6 => outside.note_resources.get_mut(0).unwrap().connector = 2,
            _ => unreachable!(),
        }
        draft.authority = Some(outside);
        let prediction = admission::amendment(&offer, &draft, &before);
        assert_eq!(
            prediction,
            if dimension < 4 {
                Admission::Proposed { to: Recipient::Party(9) }
            } else {
                Admission::Refused(Refusal::NotOffered)
            }
        );
    }
    // Current task facts may already be wider than the role; narrowing remains direct.
    let mut existing = facts;
    existing.delegation.depth = 9;
    let mut narrower = existing.clone();
    narrower.spend = 19;
    draft.authority = Some(narrower);
    assert_eq!(admission::amendment(&offer, &draft, &OwnerAuthorityDocument::complete(existing)), Admission::Allowed);
}

#[test]
fn bounded_amendment_keeps_every_supplied_policy_recipient_in_order() {
    let (mut domain, _) = live();
    let mut row = task();
    let mut roles = List::with_capacity(2);
    roles.push(7).unwrap();
    roles.push(2).unwrap();
    let recipient = Recipient::Roles { project: 1, roles };
    let mut direct = row.authority.clone();
    direct.authority.spend = 15;
    let mut ceiling = row.authority.clone();
    ceiling.authority.spend = 100;
    row.acts = offered(
        ActKind::Amend,
        Admission::Bounded {
            within: Bounds::Amend {
                budget_left: 5,
                implications: List::with_capacity(0),
                grants: List::with_capacity(0),
                authority: Box::new(direct),
                ceiling: Box::new(ceiling),
            },
            beyond: Beyond::Proposed { to: recipient.clone() },
        },
    );
    let id = install(&mut domain, Body::Task(Box::new(row.clone())));
    intend(&mut domain, id, ActKind::Amend);
    let mut draft = change();
    let mut after = row.authority.whole().unwrap();
    after.spend = 16;
    draft.authority = Some(after);
    drive(&mut domain, Time::ZERO, Event::Act { action: Act::Edit { field: Purpose::Reason, text: text(b"") } });
    drive(&mut domain, Time::ZERO, Event::Act { action: Act::Change { change: Box::new(draft.clone()) } });
    assert_eq!(domain.confirmation().unwrap().proposed, Some(recipient));
    assert!(domain.confirmation().unwrap().missing.is_none());
    assert_eq!(
        sent(&drive(&mut domain, Time::ZERO, Event::Act { action: Act::Confirm })),
        &Ask::Propose {
            project: 1,
            action: ProposeAction::Amendment { task: 8, revision: row.spec_revision, change: Box::new(draft) },
            reason: text(b""),
        }
    );
}

#[test]
fn person_verdict_choices_preserve_full_codes_and_refuse_missing_machine_fields() {
    for required in [false, true] {
        let (mut domain, _) = live();
        let mut row = person();
        let mut options = List::with_capacity(1);
        let mut fields = List::with_capacity(1);
        if required {
            fields.push(FieldRule { name: b"state".as_slice().into(), max: 8 }).unwrap();
        }
        options
            .push(OptionLine { number: u32::MAX, words: text(b"choice"), words_max: 32, followups: 0, fields })
            .unwrap();
        row.contract = PersonContract::Choice { options };
        row.acts = offered(ActKind::Answer, Admission::Allowed);
        let id = install(&mut domain, Body::PersonTask(Box::new(row)));
        assert!(intend(&mut domain, id, ActKind::Answer).is_empty());
        assert!(drive(&mut domain, Time::ZERO, Event::Act { action: Act::Choose { option: u32::MAX } }).is_empty());
        let rows = drive(&mut domain, Time::ZERO, Event::Act { action: Act::Confirm });
        if required {
            assert!(rows.is_empty(), "machine field choice cannot send an incomplete answer");
            assert_eq!(domain.confirmation().unwrap().missing, Some(Missing::Choice));
        } else {
            assert_eq!(
                sent(&rows),
                &Ask::Answer {
                    project: 1,
                    on: AnswerOn::Person {
                        task: 8,
                        revision: Revision { value: 4 },
                        answer: PersonAnswer::Choice { option: u32::MAX, reason: text(b"") }
                    }
                }
            );
        }
    }
}

#[test]
fn approval_confirmation_keeps_the_whole_state_and_actual_optional_explanation() {
    for (reason, explanation) in [
        (None, None),
        (Some(b"".as_slice()), Some(text(b""))),
        (Some(b"explanation".as_slice()), Some(text(b"explanation"))),
    ] {
        let (mut domain, _) = live();
        let mut row = person();
        row.contract = PersonContract::Approval {
            state: text(UTF8_TEXTS[0]),
            part: Part::Resource {
                connector: 1,
                path: {
                    let mut path = List::with_capacity(1);
                    path.push(Box::<[u8]>::from(*b"state")).unwrap();
                    path
                },
            },
        };
        row.acts = offered(ActKind::Answer, Admission::Allowed);
        let id = install(&mut domain, Body::PersonTask(Box::new(row)));
        assert!(intend(&mut domain, id, ActKind::Answer).is_empty());
        if let Some(reason) = reason {
            edit(&mut domain, Purpose::Reason, reason);
        }
        assert_eq!(
            sent(&drive(&mut domain, Time::ZERO, Event::Act { action: Act::Confirm })),
            &Ask::Answer {
                project: 1,
                on: AnswerOn::Person {
                    task: 8,
                    revision: Revision { value: 4 },
                    answer: PersonAnswer::Approved { state: Box::from(UTF8_TEXTS[0]), words: explanation }
                }
            }
        );
    }
}

fn specification(words: Text) -> SpecificationDocument {
    SpecificationDocument::complete(Specification {
        words,
        parameters: List::with_capacity(0),
        inputs: List::with_capacity(0),
    })
}

#[test]
fn bounded_authority_drafts_use_literal_pattern_order_and_complete_source_facts() {
    for cuts in 0_u8..8 {
        for spend in [9_u64, 15] {
            let (mut domain, _) = live();
            let mut row = task();
            let mut segments = List::with_capacity(2);
            segments.push(Box::from(*b"env\0")).unwrap();
            let current = OwnerGrant {
                connector: u16::MAX,
                kind: u16::MAX,
                pattern: OwnerPattern { segments: segments.clone(), last: OwnerPatternEnd::Exact(Box::from(*b"a/b")) },
            };
            row.authority.authority.grants = List::with_capacity(2);
            row.authority.authority.grants.push(current.clone()).unwrap();
            row.authority.cut = cuts & 1 != 0;
            let mut direct = row.authority.clone();
            direct.authority.spend = 15;
            direct.authority.grants.get_mut(0).unwrap().pattern.last = OwnerPatternEnd::Open(Box::from(*b"a"));
            direct.cut = cuts & 2 != 0;
            let mut ceiling = direct.clone();
            ceiling.authority.spend = 100;
            ceiling.cut = cuts & 4 != 0;
            row.acts = offered(
                ActKind::Amend,
                Admission::Bounded {
                    within: Bounds::Amend {
                        budget_left: 5,
                        implications: List::with_capacity(0),
                        grants: direct.authority.grants.clone(),
                        authority: Box::new(direct),
                        ceiling: Box::new(ceiling),
                    },
                    beyond: Beyond::Proposed { to: Recipient::Party(9) },
                },
            );
            let id = install(&mut domain, Body::Task(Box::new(row.clone())));
            intend(&mut domain, id, ActKind::Amend);
            let mut after = row.authority.authority.clone();
            after.spend = spend;
            let mut draft = change();
            draft.authority = Some(after);
            drive(
                &mut domain,
                Time::ZERO,
                Event::Act { action: Act::Edit { field: Purpose::Reason, text: text(b"") } },
            );
            drive(&mut domain, Time::ZERO, Event::Act { action: Act::Change { change: Box::new(draft.clone()) } });
            let refused = cuts & 1 != 0 || spend > 10 && cuts & 6 != 0;
            if refused {
                assert_eq!(domain.confirmation().unwrap().missing, Some(Missing::Refused { why: Refusal::NotOffered }));
                assert!(drive(&mut domain, Time::ZERO, Event::Act { action: Act::Confirm }).is_empty());
            } else {
                assert!(domain.confirmation().unwrap().missing.is_none());
                assert_eq!(domain.confirmation().unwrap().proposed, None);
                let out = drive(&mut domain, Time::ZERO, Event::Act { action: Act::Confirm });
                assert_eq!(
                    sent(&out),
                    &Ask::Amend {
                        reason: text(b""),
                        project: 1,
                        task: 8,
                        revision: Some(row.spec_revision),
                        change: Box::new(draft),
                    }
                );
            }
            assert_eq!(row.authority.authority.grants.get(0), Some(&current));
            assert_eq!(row.authority.authority.grants.capacity(), 2);
        }
    }
}

#[test]
fn bounded_authority_uses_only_supplied_connector_kind_implications_and_preserves_the_whole_offer() {
    for spend in [9_u64, 15] {
        for relation in 0_u8..4 {
            let (mut domain, _) = live();
            let mut row = task();
            let pattern =
                OwnerPattern { segments: List::with_capacity(0), last: OwnerPatternEnd::Exact(Box::from(*b"a/b\0")) };
            let grant = OwnerGrant { connector: u16::MAX, kind: u16::MAX, pattern };
            row.authority.authority.grants = List::with_capacity(1);
            row.authority.authority.grants.push(grant).unwrap();
            let mut after = row.authority.authority.clone();
            after.spend = spend;
            after.grants.get_mut(0).unwrap().kind = 0;
            let mut direct = row.authority.clone();
            direct.authority.spend = 15;
            let mut ceiling = direct.clone();
            ceiling.authority.spend = 100;
            ceiling.authority.grants = after.grants.clone();
            let entry = OwnerKindImplication {
                connector: if relation == 3 { 0 } else { u16::MAX },
                kind: if relation == 2 { 0 } else { u16::MAX },
                implies: if relation == 2 { u16::MAX } else { 0 },
            };
            let mut implications = List::with_capacity(2);
            if relation != 0 {
                implications.push(entry).unwrap();
                implications.push(entry).unwrap();
            }
            let within = Bounds::Amend {
                budget_left: 5,
                grants: direct.authority.grants.clone(),
                implications: implications.clone(),
                authority: Box::new(direct),
                ceiling: Box::new(ceiling),
            };
            row.acts = offered(
                ActKind::Amend,
                Admission::Bounded { within: within.clone(), beyond: Beyond::Proposed { to: Recipient::Party(9) } },
            );
            let id = install(&mut domain, Body::Task(Box::new(row.clone())));
            intend(&mut domain, id, ActKind::Amend);
            let mut draft = change();
            draft.authority = Some(after);
            drive(
                &mut domain,
                Time::ZERO,
                Event::Act { action: Act::Edit { field: Purpose::Reason, text: text(b"") } },
            );
            drive(&mut domain, Time::ZERO, Event::Act { action: Act::Change { change: Box::new(draft.clone()) } });
            let confirmation = domain.confirmation().unwrap();
            assert!(confirmation.missing.is_none());
            assert_eq!(confirmation.proposed, if relation == 1 { None } else { Some(Recipient::Party(9)) });
            let out = drive(&mut domain, Time::ZERO, Event::Act { action: Act::Confirm });
            assert_eq!(
                sent(&out),
                &if relation == 1 {
                    Ask::Amend {
                        project: 1,
                        task: 8,
                        revision: Some(row.spec_revision),
                        change: Box::new(draft),
                        reason: text(b""),
                    }
                } else {
                    Ask::Propose {
                        project: 1,
                        action: ProposeAction::Amendment {
                            task: 8,
                            revision: row.spec_revision,
                            change: Box::new(draft),
                        },
                        reason: text(b""),
                    }
                }
            );
            let Bounds::Amend { implications: retained, .. } = within else { panic!("whole amendment offer") };
            assert_eq!(retained, implications);
            assert_eq!(retained.capacity(), 2);
        }
    }
}

mod amendment_reason;

mod task_hold;

mod owner_escalation;

const UTF8_TEXTS: [&[u8]; 1] = [b"\0~\x01"];

#[test]
fn an_amendment_beyond_the_offer_says_to_whom_it_will_be_proposed() {
    let (mut domain, _) = live();
    let mut row = task();
    row.acts = offered(ActKind::Amend, Admission::Proposed { to: Recipient::Party(9) });
    let id = install(&mut domain, Body::Task(Box::new(row)));
    intend(&mut domain, id, ActKind::Amend);
    edit(&mut domain, Purpose::Reason, b"more work");
    drive(&mut domain, Time::ZERO, Event::Act { action: Act::Change { change: Box::new(change()) } });
    assert_eq!(domain.confirmation().unwrap().proposed, Some(Recipient::Party(9)));
    assert_eq!(
        sent(&drive(&mut domain, Time::ZERO, Event::Act { action: Act::Confirm })),
        &Ask::Propose {
            project: 1,
            action: ProposeAction::Amendment { task: 8, revision: Revision { value: 9 }, change: Box::new(change()) },
            reason: text(b"more work"),
        }
    );
}

fn intend_amend_release(domain: &mut Fixture, id: Id<Object>) -> Box<[Request]> {
    drive(domain, Time::ZERO, Event::Act { action: Act::AmendRelease { object: id.token() } })
}

fn bounded_grant() -> OwnerGrant {
    OwnerGrant {
        connector: 1,
        kind: 2,
        pattern: OwnerPattern { segments: List::with_capacity(0), last: OwnerPatternEnd::Exact(Box::from(*b"listed")) },
    }
}

mod compound;

mod cancel_reason;
