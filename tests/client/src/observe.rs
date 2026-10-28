//! Shared jig observations and the Test application's family vocabulary.
#![allow(clippy::wildcard_imports, reason = "boundary adapter covers the owner vocabulary")]
use crate::harness::{CardKey, CardSeen, Receipt, Seen, Sent};
use jig_client::contract::{AnswerOn, DecideOn};
use jig_client_domain::contract::*;
use jig_client_test_domain::{self as domain, Event, Request, family};
use jig_client_view::Owner;
use skein_fake_person::{Face, Find, Found, TreeFace};
use skein_lib::{RequestEnvelope, Token};
use skein_ui::{Key, NodeId, Role, Tree};

#[must_use]
pub fn key(object: ObjectKey) -> CardKey {
    let (kind, a, b) = match object {
        ObjectKey::Task(task) => (0, task, 0),
        ObjectKey::Proposal(p) => match p.proposer {
            Proposer::Task(task) => (1, task, p.number),
            Proposer::Party(party) => (29, party, p.number),
        },
        ObjectKey::Escalation(task) => (2, task, 0),
        ObjectKey::Question { task, message } => (3, task, message),
        ObjectKey::PersonTask(task) => (4, task, 0),
        ObjectKey::Result(task) => (5, task, 0),
        ObjectKey::Note { project, note } => (6, u64::from(project), note),
    };
    CardKey::of(Key { owner: Owner::Jig.opaque(), kind, a, b })
}
#[must_use]
pub fn sent(request: &Request) -> Option<Sent> {
    let Request::Client {
        request: jig_client::Request::Down { down: jig_client::Down::Send { attempt, key: k, request: ask, .. } },
    } = request
    else {
        return None;
    };
    let (card, revision, words) = match ask {
        jig_client::Ask::Jig(ask) if matches!(ask.as_ref(), Ask::Seen { .. }) => return None,
        jig_client::Ask::Jig(ask) => return Some(jig_sent(*attempt, *k, ask)),
        jig_client::Ask::Family { record, .. } => {
            let ask = crate::test::family_request(record);
            let family::Ask::Touch { project, item, revision, .. } = ask else {
                unreachable!("decoded optional request")
            };
            (
                Some(CardKey::of(jig_client_test_view::family_key(&family::Subject {
                    scope: family::Scope::Project(project),
                    kind: family::Kind::Item(item),
                }))),
                Some((0, revision.value)),
                None,
            )
        }
    };
    Some(Sent { attempt: *attempt, key: *k, card, revision, words })
}
/// Observe a keyed jig send without consulting an application's root state.
#[must_use]
pub fn jig_sent(attempt: skein_lib::Token, request_key: [u8; 16], ask: &Ask) -> Sent {
    let (card, revision, words) = match ask {
        Ask::Compound { operations, .. } => match operations.as_slice().first() {
            Some(jig_client_domain::contract::Operation::Message { task, words, .. }) => {
                (None, None, Some((*task, words.bytes().to_vec())))
            }
            Some(jig_client_domain::contract::Operation::Amend { task, revision, .. }) => {
                (Some(key(ObjectKey::Task(*task))), revision.map(|v| (1, v.value)), None)
            }
            _ => (None, None, None),
        },
        Ask::Message { task, words, .. } | Ask::Answer { on: AnswerOn::Question { task, words, .. }, .. } => {
            (None, None, Some((*task, words.bytes().to_vec())))
        }
        Ask::Answer { on: AnswerOn::Person { task, revision, .. }, .. } => {
            (Some(key(ObjectKey::PersonTask(*task))), Some((3, revision.value)), None)
        }
        Ask::Release { task, revision, .. } => (Some(key(ObjectKey::Task(*task))), Some((2, revision.value)), None),
        Ask::Decide { on: DecideOn::Escalation { task, revision, .. }, .. } => {
            (Some(key(ObjectKey::Escalation(*task))), Some((2, revision.value)), None)
        }
        Ask::Amend { task, revision, .. } => {
            (Some(key(ObjectKey::Task(*task))), revision.map(|value| (1, value.value)), None)
        }
        Ask::EditNote { project, note, revision, .. } | Ask::DeleteNote { project, note, revision, .. } => {
            (Some(key(ObjectKey::Note { project: *project, note: *note })), Some((4, revision.value)), None)
        }
        Ask::Decide { on: DecideOn::Proposal { proposal, .. }, .. } => {
            (Some(key(ObjectKey::Proposal(*proposal))), None, None)
        }
        Ask::Propose {
            action: jig_client_domain::contract::ProposeAction::Amendment { task, revision, .. }, ..
        } => (Some(key(ObjectKey::Task(*task))), Some((1, revision.value)), None),
        Ask::Propose { action: jig_client_domain::contract::ProposeAction::ReleasePlaced { .. }, .. }
        | Ask::Propose { action: jig_client_domain::contract::ProposeAction::Settings(_), .. }
        | Ask::Propose { action: jig_client_domain::contract::ProposeAction::Batch { .. }, .. }
        | Ask::Withdraw { .. }
        | Ask::Create { .. }
        | Ask::Take { .. }
        | Ask::HandBack { .. }
        | Ask::Stop { .. }
        | Ask::Cancel { .. }
        | Ask::Move { .. }
        | Ask::Seen { .. }
        | Ask::ChangePolicy { .. }
        | Ask::SetRoles { .. }
        | Ask::MakeService { .. }
        | Ask::SignIn { .. }
        | Ask::AccountChange { .. }
        | Ask::SignOut { .. }
        | Ask::Rename { .. }
        | Ask::Hold { .. }
        | Ask::Lift { .. }
        | Ask::ChangeSettings(_)
        | Ask::MoveToNewest(_)
        | Ask::MakeProject { .. }
        | Ask::SetPool { .. } => (None, None, None),
    };
    Sent { attempt, key: request_key, card, revision, words }
}
fn task(t: &Task) -> Receipt {
    let mut revisions = vec![(1, t.spec_revision.value)];
    if let Some(hold) = &t.hold {
        revisions.push((2, hold.revision().value));
    }
    Receipt::Card { key: key(ObjectKey::Task(t.chip.task)), revisions }
}
fn read(v: &ReadValue) -> Vec<Receipt> {
    let mut receipts = read_cards(v);
    receipts.push(Receipt::Document { value: Box::new(v.clone()) });
    receipts
}
fn read_cards(v: &ReadValue) -> Vec<Receipt> {
    match v {
        ReadValue::Task(t) => vec![task(t)],
        ReadValue::Me(f) => frame(f, None),
        ReadValue::Escalation(e) => vec![Receipt::Card {
            key: key(ObjectKey::Escalation(e.task.task)),
            revisions: vec![(1, e.steering.revision.value), (2, e.hold.revision.value)],
        }],
        ReadValue::OwnerEscalation(e) => {
            let mut revisions = vec![(1, e.task.spec_revision.value)];
            if let Some(hold) = &e.task.hold {
                revisions.push((2, hold.revision().value));
            }
            vec![Receipt::Card { key: key(ObjectKey::Escalation(e.task.chip.task)), revisions }]
        }
        ReadValue::PersonTask(p) => {
            vec![Receipt::Card { key: key(ObjectKey::PersonTask(p.task.task)), revisions: vec![(3, p.revision.value)] }]
        }
        ReadValue::OwnerPersonTask(p) => {
            vec![Receipt::Card { key: key(ObjectKey::PersonTask(p.task.task)), revisions: vec![(3, p.revision.value)] }]
        }
        ReadValue::Notes(p) => p
            .rows
            .iter()
            .filter_map(|n| match &n.scope {
                NoteScope::Project(project) | NoteScope::Resource { project, .. } | NoteScope::Task { project, .. } => {
                    Some(Receipt::Card {
                        key: key(ObjectKey::Note { project: *project, note: n.number }),
                        revisions: vec![(4, n.revision.value)],
                    })
                }
                NoteScope::Deployment => None,
            })
            .collect(),
        ReadValue::Tasks(_)
        | ReadValue::Proposal(_)
        | ReadValue::Historical(_)
        | ReadValue::OwnerProposal(_)
        | ReadValue::Question(_)
        | ReadValue::Result(_)
        | ReadValue::Ended(_)
        | ReadValue::History(_)
        | ReadValue::Activity(_)
        | ReadValue::Turn(_)
        | ReadValue::Inbox(_)
        | ReadValue::OwnerInbox(_)
        | ReadValue::Holds(_)
        | ReadValue::Spend(_)
        | ReadValue::OwnerEnded(_)
        | ReadValue::OwnerSpend(_)
        | ReadValue::OwnerProject(_)
        | ReadValue::Project(_)
        | ReadValue::Settings(_)
        | ReadValue::Setting(_)
        | ReadValue::Revision(_)
        | ReadValue::Revisions(_)
        | ReadValue::Deployment(_)
        | ReadValue::OwnerDeployment(_) => vec![],
    }
}
fn frame(frame: &Me, watch: Option<Token>) -> Vec<Receipt> {
    vec![Receipt::Party { party: frame.party.number, watch }]
}
fn family(doc: &family::Document) -> Receipt {
    let revision = match doc {
        family::Document::Item(item) => Some(item.revision),
        family::Document::Part(part) => Some(part.parameters.map_or(part.revision, |p| p.revision)),
        family::Document::Connector(_) => doc.revision(),
    };
    Receipt::Card {
        key: CardKey::of(jig_client_test_view::family_key(&doc.subject())),
        revisions: revision.map_or_else(Vec::new, |revision| vec![(0, revision.value)]),
    }
}
#[must_use]
pub fn received(event: &Event) -> Vec<Receipt> {
    match event {
        Event::Client { event: jig_client::Event::Up { up } } => up_received(up),
        Event::Start { .. } => vec![Receipt::Started],
        Event::Jig(event) => jig_received(event),
        Event::First(
            domain::first::Event::Read { result: family::ReadResult::Value(doc), .. }
            | domain::first::Event::Watch { event: family::WatchEvent::Delivered(doc), .. },
        ) => vec![family(doc)],
        Event::SignedOut
        | Event::First(
            domain::first::Event::Read { result: family::ReadResult::SignedOut, .. }
            | domain::first::Event::Ended { end: family::WatchEnd::SignedOut, .. },
        ) => vec![Receipt::Parked],
        Event::Client { .. } | Event::Went { .. } | Event::Act { .. } | Event::First(_) => vec![],
    }
}
/// Observe delivered jig documents and sign-in state before the root callback.
#[must_use]
pub fn jig_received(event: &jig_client_view::Event) -> Vec<Receipt> {
    match event {
        jig_client_view::Event::Start { .. } => vec![Receipt::Started],
        jig_client_view::Event::Client { .. }
        | jig_client_view::Event::Went { .. }
        | jig_client_view::Event::Left
        | jig_client_view::Event::Act { .. }
        | jig_client_view::Event::Named { .. } => vec![],
    }
}
/// Observe native transport deliveries independently of the root and UI.
#[must_use]
pub fn up_received(up: &jig_client::Up) -> Vec<Receipt> {
    use jig_client::{Readout, Up};
    match up {
        Up::Answer { attempt, envelope, answer, why: None } => {
            let words = match answer {
                Some(jig_client::Answered::Jig(Answer::Done(Outcome::Owner(
                    OwnerOutcome::Said { task, message, .. } | OwnerOutcome::QuestionAnswered { task, message, .. },
                )))) => Some((*task, *message)),
                Some(jig_client::Answered::Jig(_) | jig_client::Answered::Family(_)) | None => None,
            };
            if *envelope == RequestEnvelope::SignedOut {
                vec![Receipt::Parked]
            } else {
                let mut receipts = vec![Receipt::Answer {
                    attempt: *attempt,
                    final_answer: *envelope == RequestEnvelope::Final,
                    words,
                }];
                if *envelope == RequestEnvelope::Final
                    && let Some(jig_client::Answered::Jig(answer)) = answer
                    && let Some(outcome) = crate::harness::referee::offers::outcome(answer)
                {
                    receipts.push(Receipt::DraftAnswer { attempt: *attempt, outcome });
                }
                receipts
            }
        }
        Up::Served { served: false, .. } => vec![Receipt::OutOfDate],
        Up::ReadDone { result: Readout::Jig(ReadResult::Value(value)), .. } => read(value),
        Up::Watched { event: WatchEvent::Delivered(Delivery::Task(value)), .. } => {
            read(&ReadValue::Task(value.clone()))
        }
        Up::Watched { watch, event: WatchEvent::Delivered(Delivery::Me(value)) } => {
            let mut receipts = frame(value, Some(*watch));
            receipts.push(Receipt::Document { value: Box::new(ReadValue::Me(value.clone())) });
            receipts
        }

        Up::Watched { event: WatchEvent::Delivered(Delivery::OwnerProject(value)), .. } => {
            read(&ReadValue::OwnerProject(value.clone()))
        }
        Up::Watched { event: WatchEvent::Delivered(Delivery::OwnerProposal(value)), .. } => {
            read(&ReadValue::OwnerProposal(value.clone()))
        }
        Up::WatchEnded { watch, end } => {
            let mut receipts = vec![Receipt::WatchEnded { watch: *watch }];
            if *end == WatchEnd::SignedOut {
                receipts.push(Receipt::Parked);
            }
            receipts
        }
        Up::SignedOut | Up::ReadDone { result: Readout::Jig(ReadResult::None(ReadNone::SignedOut)), .. } => {
            vec![Receipt::Parked]
        }
        Up::SignedIn
        | Up::NotKept { .. }
        | Up::Served { served: true, .. }
        | Up::FamilyServed { .. }
        | Up::FamilyDelivered { .. }
        | Up::ReadDone { .. }
        | Up::Watched { .. }
        | Up::Answer { .. } => vec![],
    }
}

fn contains(tree: &Tree, start: usize, end: usize, role: Role, name: &[u8]) -> bool {
    let Some(mut face) = TreeFace::subtree(tree, tree.nodes()[start].id) else {
        return false;
    };
    match face.find(&Find::named(role, name)) {
        Found::Node(id) => {
            let Some(at) = tree.nodes().iter().position(|n| n.id == id) else {
                return false;
            };
            at < end
                && !tree.nodes()[start + 1..=at].iter().enumerate().any(|(offset, n)| {
                    (n.key.is_some_and(card_key)
                        || (n.role == Role::Item
                            && n.key.is_some_and(|key| {
                                matches!(Owner::from_opaque(key.owner), Some(Owner::Application { .. }))
                            })))
                        && start + 1 + offset + usize::try_from(n.size).expect("subtree") > at
                })
        }
        Found::Absent | Found::Pending => false,
    }
}
fn explained(tree: &Tree, start: usize, end: usize) -> bool {
    let Some(mut face) = TreeFace::subtree(tree, tree.nodes()[start].id) else {
        return false;
    };
    let Found::Node(id) = face.find(&Find::named(Role::Status, b"Leaving")) else {
        return false;
    };
    let Some(at) = tree.nodes().iter().position(|n| n.id == id) else {
        return false;
    };
    let through = at + usize::try_from(tree.nodes()[at].size).expect("status subtree");
    through <= end && tree.nodes()[at..through].iter().any(|n| !n.text.is_empty())
}
fn card_key(k: Key) -> bool {
    match Owner::from_opaque(k.owner) {
        Some(Owner::Jig) => k.kind <= 6 || k.kind == 29,
        Some(Owner::Application { number: connector }) => {
            connector == domain::FIRST && ((10..=16).contains(&k.kind) || k.kind == 19 || k.kind == 20)
        }
        None => false,
    }
}
#[must_use]
pub fn card(tree: &Tree, node: NodeId) -> Option<CardKey> {
    enclosing_card(tree, node, card_key)
}
/// Find a jig card using only its stable public key and tree ancestry.
#[must_use]
pub fn jig_card(tree: &Tree, node: NodeId) -> Option<CardKey> {
    enclosing_card(tree, node, |key| key.owner == Owner::Jig.opaque() && (key.kind <= 6 || key.kind == 29))
}
fn enclosing_card(tree: &Tree, node: NodeId, admits: fn(Key) -> bool) -> Option<CardKey> {
    let at = tree.nodes().iter().position(|n| n.id == node)?;
    tree.nodes()
        .iter()
        .enumerate()
        .take(at + 1)
        .filter(|(i, n)| *i + usize::try_from(n.size).expect("subtree") > at)
        .filter_map(|(_, n)| n.key.filter(|k| admits(*k)))
        .next_back()
        .map(CardKey::of)
}
#[must_use]
pub fn seen(tree: &Tree) -> Vec<Seen> {
    visible(tree, true)
}
/// Observe jig cards, delivered words and shell state in a composed public tree.
#[must_use]
pub fn jig_seen(tree: &Tree) -> Vec<Seen> {
    visible(tree, false)
}
fn frame_link(tree: &Tree) -> Option<bool> {
    let (at, frame) = tree.nodes().iter().enumerate().find(|(_, node)| {
        node.role == Role::Region && node.region == Some(jig_client_view::regions::JigRegion::Link.region())
    })?;
    let end = at + usize::try_from(frame.size).expect("frame subtree");
    let mut child = at + 1;
    // The frame's own status is current; notices and navigation slots can
    // retain other link words. Read direct frame children, not their descendants.
    while child < end {
        let node = &tree.nodes()[child];
        if node.role == Role::Status {
            match node.name.as_deref() {
                Some(b"Live") => return Some(true),
                Some(b"Offline") => return Some(false),
                _ => {}
            }
        }
        child += usize::try_from(node.size).expect("frame child subtree");
    }
    None
}
fn visible(tree: &Tree, include_test_family: bool) -> Vec<Seen> {
    let mut seen = Vec::new();
    if [b"Sign in".as_slice(), b"Someone else's work", b"Page missing", b"Client out of date"].iter().any(|name| {
        tree.nodes().iter().any(|n| {
            n.role == Role::Heading { level: 1 }
                && TreeFace::subtree(tree, n.id)
                    .is_some_and(|mut face| matches!(face.find(&Find::text(name)), Found::Node(_)))
        })
    }) {
        seen.push(Seen::HiddenPage);
    }
    if tree.nodes().iter().any(|node| {
        node.role == Role::Heading { level: 1 }
            && TreeFace::subtree(tree, node.id)
                .is_some_and(|mut face| matches!(face.find(&Find::text(b"Sign in")), Found::Node(_)))
    }) {
        seen.push(Seen::SignedOut);
    }
    if let Some(live) = frame_link(tree) {
        seen.push(Seen::Link { live });
    }
    for (at, node) in tree.nodes().iter().enumerate() {
        let Some(k) = node.key else {
            continue;
        };
        let end = at + usize::try_from(node.size).expect("subtree");
        // Plan tree items reuse object identity, but only Item nodes are cards.
        if (k.owner == Owner::Jig.opaque() && (k.kind <= 6 || k.kind == 29) && node.role == Role::Item)
            || (include_test_family && k.owner != Owner::Jig.opaque() && card_key(k))
        {
            let deciding = contains(tree, at, end, Role::Status, b"In flight")
                || contains(tree, at, end, Role::Status, b"Retrying")
                || contains(tree, at, end, Role::Status, b"Parked")
                || contains(tree, at, end, Role::Status, b"Being decided");
            let leaving = contains(tree, at, end, Role::Status, b"Leaving");
            let looking = contains(tree, at, end, Role::Status, b"Leaving")
                && contains(tree, at, end, Role::Text, b"Finding how this ended");
            let done = contains(tree, at, end, Role::Status, b"Touched")
                || contains(tree, at, end, Role::Status, b"Moved to revision")
                || contains(tree, at, end, Role::Status, b"Gone")
                || (include_test_family && contains(tree, at, end, Role::Status, b"Application outcome"));
            let state = if leaving && !looking {
                CardSeen::Left { reason: explained(tree, at, end) }
            } else if deciding || looking {
                CardSeen::Deciding
            } else if done {
                CardSeen::Done
            } else {
                CardSeen::Waiting
            };
            seen.push(Seen::Card { key: CardKey::of(k), state, revision: None });
        }
        if k.owner == Owner::Jig.opaque()
            && (k.kind == 25 || k.kind == 26)
            && !contains(tree, at, end, Role::Status, b"Being sent")
        {
            let status = tree.nodes()[at..end].iter().position(|n| {
                n.role == Role::Status
                    && (contains(tree, at, end, Role::Status, b"Sent")
                        || contains(tree, at, end, Role::Status, b"Read"))
            });
            if let Some(status) = status
                && let Some(text) = tree.nodes()[at..at + status].iter().rev().find(|n| !n.text.is_empty())
            {
                seen.push(Seen::Words {
                    task: k.a,
                    message: (k.kind == 25).then_some(k.b),
                    words: text.text.iter().flat_map(|r| r.text.iter().copied()).collect(),
                });
            }
        }
    }
    seen
}

#[cfg(test)]
#[path = "observe/link_tests.rs"]
mod link_tests;

/// Only decoded public person acts explain changing the displayed window (QUESTIONS 34).
#[must_use]
pub fn window_changed(event: &Event) -> bool {
    let act = match event {
        Event::Act { action: domain::Action::Jig(act) } | Event::Jig(jig_client_view::Event::Act { action: act }) => {
            act
        }
        Event::Start { .. }
        | Event::Went { .. }
        | Event::Act { action: domain::Action::SignIn | domain::Action::First(_) }
        | Event::Client { event: jig_client::Event::Up { up: jig_client::Up::Answer { .. } } }
        | Event::Jig(_)
        | Event::First(_)
        | Event::SignedOut
        | Event::Client { .. } => return false,
    };
    jig_window_changed(act)
}
/// Only decoded jig person acts explain a changed displayed window (QUESTIONS 34).
#[must_use]
pub fn jig_window_changed(act: &jig_client_view::action::Act) -> bool {
    // Settings review and explicit service role selection keep the current windows.
    if matches!(
        act,
        jig_client_view::action::Act::ProjectSettings { .. } | jig_client_view::action::Act::ProjectServiceRole { .. }
    ) {
        return false;
    }
    matches!(
        act,
        jig_client_view::action::Act::Go { .. }
            | jig_client_view::action::Act::Fold { .. }
            | jig_client_view::action::Act::FoldCall { .. }
            | jig_client_view::action::Act::FoldCurrentCall { .. }
            | jig_client_view::action::Act::Filter { .. }
            | jig_client_view::action::Act::More { .. }
            | jig_client_view::action::Act::FilterInbox { .. }
            | jig_client_view::action::Act::FilterTasks { .. }
            | jig_client_view::action::Act::FilterNotes { .. }
            | jig_client_view::action::Act::FilterPlan { .. }
            | jig_client_view::action::Act::HistoryScope { .. }
            | jig_client_view::action::Act::OpenEnded { .. }
            | jig_client_view::action::Act::OpenUpdates
    )
}

#[cfg(test)]
mod tests {
    use super::{CardKey, CardSeen, Key, Owner, Role, Seen, jig_seen};
    use jig_client_domain::contract as c;
    use skein_lib::Queue;
    use skein_ui::View;

    const ATTEMPT: skein_lib::Token = skein_lib::Token::new(7);
    const REQUEST_KEY: [u8; 16] = [3; 16];
    const PROJECT: u32 = 1;
    const TASK: u64 = 8;

    fn send(ask: jig_client::Ask) -> jig_client_test_domain::Request {
        jig_client_test_domain::Request::Client {
            request: jig_client::Request::Down {
                down: jig_client::Down::Send { attempt: ATTEMPT, key: REQUEST_KEY, request: ask, value: None },
            },
        }
    }

    #[test]
    fn top_level_seen_has_no_retained_answer_observation() {
        for through in [c::SeenThrough::Results { through: 1 }, c::SeenThrough::Conversation { task: TASK, through: 1 }]
        {
            assert!(
                super::sent(&send(jig_client::Ask::Jig(Box::new(c::Ask::Seen { through })))).is_none(),
                "{through:?}"
            );
        }
    }

    #[test]
    fn ordinary_requests_without_cards_keep_their_observations() {
        use crate::test::list;
        let reason = c::Text::new(b"because".as_slice().into(), 32).expect("fixture text");
        let name = c::Text::new(b"Tally".as_slice().into(), 32).expect("fixture name");
        let asks = [
            c::Ask::Create { project: PROJECT, tasks: list([]) },
            c::Ask::Take { project: PROJECT, task: TASK },
            c::Ask::HandBack { project: PROJECT, task: TASK },
            c::Ask::Stop { project: PROJECT, task: TASK },
            c::Ask::Cancel { project: PROJECT, task: TASK, reason: reason.clone(), grace: None },
            c::Ask::Move { project: PROJECT, task: TASK, reason: reason.clone() },
            c::Ask::Withdraw { project: PROJECT, proposal: 1 },
            c::Ask::Propose { project: PROJECT, action: c::ProposeAction::Batch { tasks: list([]) }, reason },
            c::Ask::ChangePolicy { project: PROJECT, edit: c::PolicyEdit::ProjectSpend { period_spend: 10 } },
            c::Ask::SetRoles { project: PROJECT, holdings: list([]) },
            c::Ask::MakeService {
                project: PROJECT,
                name: c::Text::new(b"service".as_slice().into(), 32).expect("fixture text"),
                role: c::HeldRole::Member,
            },
            c::Ask::Rename { project: PROJECT, name: name.clone() },
            c::Ask::MakeProject { name, policy: 1 },
            c::Ask::SetPool { project: PROJECT, party: 1, budget: 10 },
            c::Ask::SignIn { holder: c::AccountHolder::Own, provider: 1 },
            c::Ask::AccountChange {
                holder: c::AccountHolder::Own,
                change: c::AccountChange::CheckProgram { source: 1 },
            },
            c::Ask::SignOut { holder: c::AccountHolder::Own, source: 1 },
            c::Ask::Compound {
                project: PROJECT,
                operations: list([
                    c::Operation::Stop { project: PROJECT, task: TASK },
                    c::Operation::Message {
                        project: PROJECT,
                        task: TASK,
                        words: c::Text::new(b"continue".as_slice().into(), 32).expect("fixture text"),
                    },
                ]),
            },
        ];
        for ask in asks {
            let sent =
                super::sent(&send(jig_client::Ask::Jig(Box::new(ask.clone())))).expect("ordinary request observed");
            assert_eq!(
                (sent.attempt, sent.key, sent.card, sent.revision, sent.words),
                (ATTEMPT, REQUEST_KEY, None, None, None),
                "{ask:?}"
            );
        }
    }

    #[test]
    fn family_requests_keep_their_card_and_revision() {
        let (project, value) =
            jig_client_test_domain::family::wire::request(&jig_client_test_domain::family::Ask::Touch {
                project: PROJECT,
                item: TASK,
                revision: c::Revision { value: 5 },
                parameters: None,
            });
        let sent = super::sent(&send(jig_client::Ask::Family {
            scope: c::Scope::Project(project),
            record: jig_client::Family {
                family: 1,
                version: jig_client_test_protocol_wire::VERSION,
                bytes: jig_client_test_protocol_wire::codec::family::request_record(project, value),
            },
        }))
        .expect("family request observed");
        let card = CardKey::of(jig_client_test_view::family_key(&jig_client_test_domain::family::Subject {
            scope: jig_client_test_domain::family::Scope::Project(PROJECT),
            kind: jig_client_test_domain::family::Kind::Item(TASK),
        }));
        assert_eq!(
            (sent.attempt, sent.key, sent.card, sent.revision, sent.words),
            (ATTEMPT, REQUEST_KEY, Some(card), Some((0, 5)), None)
        );
    }

    #[test]
    fn a_nested_application_status_cannot_decide_a_jig_card() {
        let limits = skein_ui::Limits {
            nodes: 16,
            depth: 4,
            text_per_node: 64,
            text_bytes: 1024,
            drawing_bytes: 0,
            styles_per_node: 0,
            patches: 128,
            bindings: 0,
            slots: 0,
            markdown_depth: 1,
            scene_marks: 0,
            scene_bytes: 0,
        };
        let mut view = View::new(&limits);
        let mut builder = view.builder();
        let card = Key { owner: Owner::Jig.opaque(), kind: 0, a: 9, b: 0 };
        builder.open(Role::Page);
        builder.open(Role::Item);
        builder.key(card);
        builder.open(Role::Item);
        builder.key(Key { owner: (Owner::Application { number: 77 }).opaque(), kind: 2, a: 1, b: 5 });
        builder.open(Role::Status);
        builder.name(b"Gone");
        builder.close();
        builder.close();
        builder.close();
        builder.close();
        view.finish(builder, 1, &mut Queue::with_capacity(limits.patches));
        let seen = jig_seen(view.tree());
        assert!(
            matches!(seen.as_slice(), [Seen::Card { key, state: CardSeen::Waiting, revision: None }] if *key == CardKey::of(card))
        );
    }
    #[test]
    fn public_observations_keep_equal_task_and_party_proposal_cards_and_requests_distinct() {
        use jig_client_domain::contract::{Ask, Decision, ObjectKey, ProposalKey, Proposer};
        let limits = skein_ui::Limits {
            nodes: 16,
            depth: 4,
            text_per_node: 64,
            text_bytes: 1024,
            drawing_bytes: 0,
            styles_per_node: 0,
            patches: 128,
            bindings: 0,
            slots: 0,
            markdown_depth: 1,
            scene_marks: 0,
            scene_bytes: 0,
        };
        let mut view = View::new(&limits);
        let mut builder = view.builder();
        builder.open(Role::Page);
        let mut expected = Vec::new();
        for proposer in [Proposer::Task(u64::MAX), Proposer::Party(u64::MAX)] {
            let proposal = ProposalKey { proposer, number: u64::MAX };
            let card = super::key(ObjectKey::Proposal(proposal));
            let kind = match proposer {
                Proposer::Task(_) => 1,
                Proposer::Party(_) => 29,
            };
            assert_eq!(card, CardKey::of(Key { owner: Owner::Jig.opaque(), kind, a: u64::MAX, b: u64::MAX }));
            let sent = super::jig_sent(
                skein_lib::Token::new(1),
                [7; 16],
                &Ask::Decide {
                    project: 4,
                    on: jig_client_domain::contract::DecideOn::Proposal {
                        proposal,
                        decision: Decision::Accepted,
                        running: None,
                    },
                },
            );
            assert_eq!(sent.card, Some(card.clone()));
            builder.open(Role::Item);
            builder.key(Key { owner: Owner::Jig.opaque(), kind, a: u64::MAX, b: u64::MAX });
            builder.text(skein_ui::Run::Plain, b"supplied proposal");
            builder.close();
            expected.push(Seen::Card { key: card, state: CardSeen::Waiting, revision: None });
        }
        builder.close();
        view.finish(builder, 1, &mut Queue::with_capacity(limits.patches));
        assert_ne!(
            super::key(ObjectKey::Proposal(ProposalKey { proposer: Proposer::Task(u64::MAX), number: u64::MAX })),
            super::key(ObjectKey::Proposal(ProposalKey { proposer: Proposer::Party(u64::MAX), number: u64::MAX }))
        );
        assert_eq!(jig_seen(view.tree()), expected);
        for node in view.tree().nodes().iter().filter(|node| node.role == Role::Item) {
            assert_eq!(super::jig_card(view.tree(), node.id), node.key.map(CardKey::of));
        }
    }
    #[test]
    fn public_owner_answer_receipts_keep_original_attempt_envelope_and_exact_message_ids() {
        use super::{Receipt, received};
        use jig_client::Answered;
        use jig_client_domain::contract::{Answer, Outcome, OwnerOutcome};
        use jig_client_test_domain::Event;
        use skein_lib::{RequestEnvelope, Token};
        let cases = [
            (OwnerOutcome::Said { task: u64::MAX, message: 37 }, Some((u64::MAX, 37))),
            (OwnerOutcome::Said { task: 41, message: u64::MAX }, Some((41, u64::MAX))),
            (OwnerOutcome::QuestionAnswered { task: 43, question: 47, message: 53 }, Some((43, 53))),
        ];
        for (outcome, expected) in cases {
            for envelope in [RequestEnvelope::Final, RequestEnvelope::Again, RequestEnvelope::Lost] {
                let event = Event::Client {
                    event: jig_client::Event::Up {
                        up: jig_client::Up::Answer {
                            attempt: Token::new(73),
                            envelope,
                            answer: Some(Answered::Jig(Answer::Done(Outcome::Owner(outcome.clone())))),
                            why: None,
                        },
                    },
                };
                let receipts: Vec<_> =
                    received(&event).into_iter().filter(|receipt| matches!(receipt, Receipt::Answer { .. })).collect();
                let [Receipt::Answer { attempt, final_answer, words }] = receipts.as_slice() else {
                    panic!("one original answer envelope");
                };
                assert_eq!(*attempt, Token::new(73));
                assert_eq!(*final_answer, envelope == RequestEnvelope::Final);
                assert_eq!(*words, expected);
            }
            let signed_out = Event::Client {
                event: jig_client::Event::Up {
                    up: jig_client::Up::Answer {
                        attempt: Token::new(73),
                        envelope: RequestEnvelope::SignedOut,
                        answer: Some(Answered::Jig(Answer::Done(Outcome::Owner(outcome)))),
                        why: None,
                    },
                },
            };
            assert!(matches!(received(&signed_out).as_slice(), [Receipt::Parked]));
        }
    }
}
