//! Safety and liveness from shell, tree and durable boundary observations
//! (client/domain/testing.md, 6). No rule reads a root or a binding table.
pub(crate) mod offers;
use super::{CardKey, CardSeen, ClientId, Observation, Observed, Receipt, Seen, Sent};
use skein_lib::{Duration, Token};
use skein_world::domain::{Expectations, Judge};
use std::collections::{BTreeMap, BTreeSet};

use jig_client::{
    Key,
    contract::{ActKind, AnswerOn, Ask, DecideOn, Decision, ObjectKey, Operation, ProposeAction, ReadValue},
};
use jig_client_test_kit::check::{Check, Delivered, Fault, Limits, Scene};
use skein_ui::phrases::Phrasebook;

/// Named request and link obligations, independent of attempts and reloads.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Deadline {
    /// The first send of one party's key must receive a final answer.
    Request(u64, [u8; 16]),
    /// Unreachable beyond a heartbeat must be shown offline.
    Offline(ClientId),
    /// Recovered truth must become visibly online.
    Online(ClientId),
}
type JudgeOf = Judge<Deadline, ()>;
type RequestKey = (u64, [u8; 16]);
type Card = (ClientId, CardKey);
#[derive(Debug)]
struct Pending {
    client: ClientId,
    card: Option<CardKey>,
    words: Option<(u64, Vec<u8>)>,
}
/// Seven safety rules with request and link deadlines in skein's referee.
#[derive(Debug)]
pub struct Rules {
    checks: BTreeMap<ClientId, Checks>,
    words: Phrasebook,
    offers: offers::Offers,
    heartbeat: Duration,
    request_within: Duration,
    link_within: Duration,
    clients: BTreeSet<ClientId>,
    links: BTreeMap<ClientId, bool>,
    reachable: Option<bool>,
    parked: BTreeSet<ClientId>,
    ended_watches: BTreeSet<(ClientId, Token)>,
    out_of_date: BTreeSet<ClientId>,
    sign_out_asked: BTreeSet<ClientId>,
    sign_in_ended: BTreeSet<ClientId>,
    durable: BTreeSet<u64>,
    made: BTreeMap<RequestKey, u64>,
    requests: BTreeMap<RequestKey, Pending>,
    attempts: BTreeMap<(ClientId, Token), RequestKey>,
    documents: BTreeMap<(ClientId, CardKey, u16), u64>,
    visible: BTreeMap<Card, (CardSeen, Option<u64>)>,
    answered: BTreeMap<Card, RequestKey>,
    stopped: BTreeMap<CardKey, u64>,
    reasoned: BTreeSet<Card>,
    kept: BTreeMap<(u64, u64), Vec<u8>>,
    ended: BTreeSet<u64>,
    promised: BTreeMap<(u64, u64), Vec<u8>>,
    acknowledged: BTreeMap<(u64, u64), Vec<u8>>,
}
impl Rules {
    /// Installs the world's spans; commit zero is its durable starting truth.
    #[must_use]
    pub fn new(heartbeat: Duration, request_within: Duration, link_within: Duration) -> Self {
        Self::with_words(heartbeat, request_within, link_within, &jig_client::words::english())
    }
    /// Install the application's decision vocabulary.
    #[must_use]
    pub fn with_words(
        heartbeat: Duration,
        request_within: Duration,
        link_within: Duration,
        words: &skein_ui::phrases::Language,
    ) -> Self {
        let words = crate::test::decision_phrasebook(words);
        Self {
            words,
            checks: BTreeMap::new(),
            offers: offers::Offers::default(),
            heartbeat,
            request_within,
            link_within,
            clients: BTreeSet::new(),
            links: BTreeMap::new(),
            reachable: None,
            parked: BTreeSet::new(),
            ended_watches: BTreeSet::new(),
            out_of_date: BTreeSet::new(),
            sign_out_asked: BTreeSet::new(),
            sign_in_ended: BTreeSet::new(),
            durable: BTreeSet::from([0]),
            made: BTreeMap::new(),
            requests: BTreeMap::new(),
            attempts: BTreeMap::new(),
            documents: BTreeMap::new(),
            visible: BTreeMap::new(),
            answered: BTreeMap::new(),
            stopped: BTreeMap::new(),
            reasoned: BTreeSet::new(),
            kept: BTreeMap::new(),
            ended: BTreeSet::new(),
            promised: BTreeMap::new(),
            acknowledged: BTreeMap::new(),
        }
    }
    fn truth(&mut self, observed: Observed, judge: &mut JudgeOf) {
        match observed {
            Observed::SignInEnded { client } => {
                self.sign_in_ended.insert(client);
            }
            Observed::Durable { commit, at } => {
                judge.check(at <= judge.now(), "nothing shown done before durable: future write");
                judge.check(self.durable.insert(commit), "durable commit reported once");
            }
            Observed::Made { party, key, commit } => {
                judge.check(self.durable.contains(&commit), "made answer must be durable");
                judge.check(self.made.insert((party, key), commit).is_none(), "made once: repeated party key");
            }
            Observed::Kept { task, message, words, commit } => {
                judge.check(self.durable.contains(&commit), "kept words must be durable");
                judge.check(
                    self.ended.contains(&task)
                        || self.promised.get(&(task, message)).is_none_or(|wanted| wanted == &words),
                    "nothing sent lost: committed words changed",
                );
                self.kept.insert((task, message), words);
            }
            Observed::Retained { task, messages, commit } => {
                judge.check(self.durable.contains(&commit), "retained activity must be durable");
                self.kept.retain(|(t, m), _| *t != task || messages.contains(m));
                judge.check(
                    self.ended.contains(&task)
                        || self
                            .promised
                            .keys()
                            .filter(|(t, _)| *t == task)
                            .all(|key| self.kept.get(key) == self.promised.get(key)),
                    "nothing sent lost: previously sent words left activity",
                );
            }
            Observed::Stopped { card, commit } => {
                judge.check(self.durable.contains(&commit), "stopped object must be durable");
                self.stopped.insert(card, commit);
            }
            Observed::Ended { task, commit } => {
                judge.check(self.durable.contains(&commit), "ended task must be durable");
                self.ended.insert(task);
            }
            Observed::Reachable { live } => {
                if self.reachable == Some(live) {
                    return;
                }
                self.reachable = Some(live);
                for client in &self.clients {
                    if live {
                        judge.withdraw(&Deadline::Offline(*client));
                        if !self.parked.contains(client) {
                            judge.rearm(Deadline::Online(*client), self.link_within);
                        }
                    } else {
                        judge.withdraw(&Deadline::Online(*client));
                        judge.rearm(Deadline::Offline(*client), self.heartbeat);
                    }
                    if self.links.get(client) == Some(&live) {
                        judge.meet(&if live { Deadline::Online(*client) } else { Deadline::Offline(*client) });
                    }
                }
            }
        }
    }
    /// A tab's page starts, first or on a reload: its stream has only its own
    /// heartbeat until a hello (client.md, section 8), so it tells the link
    /// within the bound of a link coming back, whatever an earlier page owed.
    fn started(&mut self, client: ClientId, judge: &mut JudgeOf) {
        judge.withdraw(&Deadline::Online(client));
        judge.withdraw(&Deadline::Offline(client));
        match self.reachable {
            Some(true) if !self.parked.contains(&client) => judge.expect(Deadline::Online(client), self.link_within),
            Some(false) => judge.expect(Deadline::Offline(client), self.link_within),
            _ => {}
        }
    }
    fn request(&mut self, client: ClientId, party: u64, sent: Sent, judge: &mut JudgeOf) {
        self.offers.requested(client, sent.attempt, judge);
        let key = (party, sent.key);
        let fresh = !self.requests.contains_key(&key);
        if fresh && let (Some(card), Some((kind, revision))) = (&sent.card, sent.revision) {
            let key = (client, card.clone());
            let shown = self
                .visible
                .get(&key)
                .and_then(|(_, revision)| *revision)
                .or_else(|| self.documents.get(&(client, card.clone(), kind)).copied());
            judge.check(
                self.visible.contains_key(&key) && shown == Some(revision),
                "what they saw is what they decided: revision differs",
            );
        }
        if fresh && let Some(card) = &sent.card {
            self.answered.remove(&(client, card.clone()));
            self.reasoned.remove(&(client, card.clone()));
        }
        if fresh || (!judge.is_pending(&Deadline::Request(party, sent.key)) && !self.made.contains_key(&key)) {
            judge.expect(Deadline::Request(party, sent.key), self.request_within);
        }
        self.requests.entry(key).or_insert(Pending { client, card: sent.card, words: sent.words });
        self.attempts.insert((client, sent.attempt), key);
    }
    fn delivered(&mut self, client: ClientId, receipt: Receipt, judge: &mut JudgeOf) {
        match receipt {
            Receipt::Document { value } => {
                self.offers.delivered(client, &value, judge);
                self.checks.entry(client).or_default().delivered(&value);
            }
            Receipt::DraftAnswer { attempt, outcome } => self.offers.answered(client, attempt, outcome, judge),
            Receipt::Card { key, revisions } => {
                for (kind, revision) in revisions {
                    self.documents
                        .entry((client, key.clone(), kind))
                        .and_modify(|prior| *prior = (*prior).max(revision))
                        .or_insert(revision);
                }
            }
            Receipt::Answer { attempt, final_answer, words } => {
                if final_answer && let Some(key) = self.attempts.get(&(client, attempt)) {
                    judge.check(
                        self.made.contains_key(key),
                        "nothing shown done before durable: final answer has no write",
                    );
                    judge.meet(&Deadline::Request(key.0, key.1));
                    if let Some((task, message)) = words
                        && let Some(Pending { words: Some((sent_task, text)), .. }) = self.requests.get(key)
                    {
                        judge.check(task == *sent_task, "sent words belong to their task");
                        self.acknowledged.insert((task, message), text.clone());
                    }
                    if let Some(pending) = self.requests.get(key)
                        && let Some(card) = &pending.card
                    {
                        self.answered.insert((client, card.clone()), *key);
                    }
                }
            }
            Receipt::Party { party, watch } => {
                if watch.is_some_and(|watch| self.ended_watches.contains(&(client, watch))) {
                    return;
                }
                if !self.out_of_date.contains(&client) && self.parked.remove(&client) && self.reachable == Some(true) {
                    judge.rearm(Deadline::Online(client), self.link_within);
                }
                for (key, pending) in &self.requests {
                    if pending.client == client && key.0 != party {
                        judge.withdraw(&Deadline::Request(key.0, key.1));
                    }
                }
            }
            Receipt::WatchEnded { watch } => {
                self.ended_watches.insert((client, watch));
            }
            Receipt::Started => {
                self.ended_watches.retain(|(owner, _)| *owner != client);
                self.out_of_date.remove(&client);
                self.sign_out_asked.remove(&client);
            }
            Receipt::OutOfDate => {
                self.out_of_date.insert(client);
                self.delivered(client, Receipt::Parked, judge);
            }
            Receipt::Parked => {
                self.parked.insert(client);
                judge.withdraw(&Deadline::Online(client));
                for (key, pending) in &self.requests {
                    if pending.client == client {
                        judge.withdraw(&Deadline::Request(key.0, key.1));
                    }
                }
            }
        }
    }
    fn shown(&mut self, client: ClientId, seen: Vec<Seen>, judge: &mut JudgeOf) {
        self.offers.shown(client);
        let mut cards = BTreeSet::new();
        if seen.contains(&Seen::HiddenPage) {
            judge.check(!seen.iter().any(|s| matches!(s, Seen::Card { .. })), "a hidden work page exposes a card");
            self.visible.retain(|(c, _), _| *c != client);
            self.reasoned.retain(|(c, _)| *c != client);
        }
        for seen in seen {
            match seen {
                Seen::Draft { key, offer } => self.offers.draft(client, key, offer),
                Seen::Card { key, state, revision } => {
                    let card = (client, key.clone());
                    cards.insert(card.clone());
                    if matches!(&state, CardSeen::Done | CardSeen::Left { .. }) {
                        let answered = self.answered.get(&card).is_some_and(|key| self.made.contains_key(key));
                        judge.check(
                            answered || self.stopped.contains_key(&key),
                            "nothing shown done before durable: card outcome has no write",
                        );
                        let reason = match &state {
                            CardSeen::Done => true,
                            CardSeen::Left { reason } => *reason,
                            CardSeen::Waiting | CardSeen::Deciding => false,
                        };
                        judge.check(reason, "a card leaves for a reason: explanation absent");
                        if reason {
                            self.reasoned.insert(card.clone());
                        }
                    }
                    if matches!(state, CardSeen::Waiting | CardSeen::Deciding) {
                        self.reasoned.remove(&card);
                    }
                    self.visible.insert(card, (state, revision));
                }
                Seen::Words { task, message, words } => {
                    let kept = match message {
                        Some(message) => self.kept.get(&(task, message)) == Some(&words),
                        None => self.acknowledged.iter().any(|(key, wanted)| {
                            key.0 == task && wanted == &words && self.kept.get(key) == Some(wanted)
                        }),
                    };
                    judge.check(self.ended.contains(&task) || kept, "nothing sent lost: acknowledged words are absent");
                    let message = message.or_else(|| {
                        self.acknowledged
                            .iter()
                            .find(|(key, wanted)| key.0 == task && *wanted == &words)
                            .map(|(key, _)| key.1)
                    });
                    if let Some(message) = message {
                        self.promised.insert((task, message), words);
                    }
                }
                Seen::Outline { .. } => judge.check(false, "the page's outline: a tree broke skein's ui.md, 14"),
                Seen::SignedOut => {
                    let allowed = !self.sign_out_asked.contains(&client) || self.sign_in_ended.contains(&client);
                    judge.check(allowed, "a sign-out told: the engine did not receive sign-out");
                    self.sign_out_asked.remove(&client);
                    self.sign_in_ended.remove(&client);
                }
                Seen::HiddenPage => {}
                Seen::Link { live } => {
                    self.links.insert(client, live);
                    judge.meet(&if live { Deadline::Online(client) } else { Deadline::Offline(client) });
                }
            }
        }
        let left: Vec<_> =
            self.visible.keys().filter(|(c, _)| *c == client).filter(|key| !cards.contains(*key)).cloned().collect();
        for key in left {
            judge.check(
                self.reasoned.remove(&key),
                format!("a card leaves for a reason: waiting card {key:?} disappeared"),
            );
            self.visible.remove(&key);
        }
    }
}
impl Expectations for Rules {
    type Seen = Observation;
    type Name = Deadline;
    type Stimulus = ();
    fn observe(&mut self, observation: Observation, judge: &mut JudgeOf) {
        match observation {
            Observation::Decision { client, key, ask, scene } => {
                if scene.is_some()
                    && let Some(offer) = self.offers.sent(client, &ask, judge)
                {
                    self.offers.draft(client, key, offer);
                }
                let fault = self.checks.entry(client).or_default().sent(key, &ask, scene.as_ref(), &self.words);
                if let Some(fault) = fault {
                    let why = match fault {
                        jig_client_test_kit::check::Fault::Undelivered { .. } => {
                            "jig's check: a decision names a revision never delivered"
                        }
                        jig_client_test_kit::check::Fault::Unseen { .. } => {
                            "jig's check: a deciding act did not show its facts"
                        }
                    };
                    judge.check(false, format!("{why}: {fault:?}; ask: {ask:?}; scene: {scene:?}"));
                }
            }
            Observation::SignOutAsked { client } => {
                self.sign_out_asked.insert(client);
            }
            Observation::Client { client } => {
                self.clients.insert(client);
                self.started(client, judge);
            }
            Observation::Truth(observed) => self.truth(observed, judge),
            Observation::Request { client, party, sent } => self.request(client, party, sent, judge),
            Observation::Delivered { client, receipt } => self.delivered(client, receipt, judge),
            Observation::Shown { client, seen } => self.shown(client, seen, judge),
            Observation::LeftPage { client } | Observation::WindowChanged { client } => {
                self.visible.retain(|(c, _), _| *c != client);
                self.reasoned.retain(|(c, _)| *c != client);
            }
            Observation::Reloaded { client } => {
                self.visible.retain(|(c, _), _| *c != client);
                self.reasoned.retain(|(c, _)| *c != client);
                self.links.remove(&client);
                self.started(client, judge);
            }
            Observation::Bounded { heap, bound } => {
                judge.check(heap <= bound, "bounded: client and view exceeded the heap envelope");
            }
        }
    }
}

#[derive(Debug)]
pub(super) struct Checks {
    check: Check,
    documents: BTreeMap<ObjectKey, ReadValue>,
}
impl Default for Checks {
    fn default() -> Self {
        Self { check: Check::new(&Limits { objects: 256, revisions: 16 }), documents: BTreeMap::new() }
    }
}
impl Checks {
    pub(super) fn delivered(&mut self, value: &ReadValue) {
        self.check.delivered(Delivered::Read(value));
        let key = match value {
            ReadValue::Task(value) => ObjectKey::Task(value.chip.task),
            ReadValue::Escalation(value) => ObjectKey::Escalation(value.task.task),
            ReadValue::OwnerEscalation(value) => ObjectKey::Escalation(value.task.chip.task),
            ReadValue::PersonTask(value) => ObjectKey::PersonTask(value.task.task),
            ReadValue::OwnerPersonTask(value) => ObjectKey::PersonTask(value.task.task),
            ReadValue::Proposal(value) => ObjectKey::Proposal(value.key),
            ReadValue::OwnerProposal(value) => ObjectKey::Proposal(value.key),
            ReadValue::Me(_)
            | ReadValue::Tasks(_)
            | ReadValue::Historical(_)
            | ReadValue::Question(_)
            | ReadValue::Result(_)
            | ReadValue::Ended(_)
            | ReadValue::History(_)
            | ReadValue::Activity(_)
            | ReadValue::Turn(_)
            | ReadValue::Inbox(_)
            | ReadValue::OwnerInbox(_)
            | ReadValue::Notes(_)
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
            | ReadValue::OwnerDeployment(_) => return,
        };
        assert!(self.documents.contains_key(&key) || self.documents.len() < 256, "world's delivered object bound");
        self.documents.insert(key, value.clone());
    }
    pub(super) fn sent(&mut self, key: Key, ask: &Ask, scene: Option<&Scene>, words: &Phrasebook) -> Option<Fault> {
        if let Some(fault) = self.check.sent(key, ask) {
            return Some(fault);
        }
        if let Some(scene) = scene {
            for (object, act) in decisions(ask) {
                let facts = self.facts(object, act, words);
                let facts: Vec<_> = facts.iter().map(Vec::as_slice).collect();
                if let Some(fault) = self.check.seen(key, act, scene, &facts) {
                    return Some(fault);
                }
            }
        }
        None
    }
    fn facts(&self, object: ObjectKey, act: ActKind, words: &Phrasebook) -> Vec<Vec<u8>> {
        let value = self.documents.get(&object).or_else(|| match object {
            ObjectKey::Task(task) => self.documents.get(&ObjectKey::Escalation(task)),
            ObjectKey::Proposal(_)
            | ObjectKey::Escalation(_)
            | ObjectKey::Question { .. }
            | ObjectKey::PersonTask(_)
            | ObjectKey::Result(_)
            | ObjectKey::Note { .. } => None,
        });
        let facts = jig_client::facts::decision(value, act);
        jig_client::words::decision(&facts, words, 0).into_boxed().into_vec().into_iter().map(Vec::from).collect()
    }
}

fn decision(value: &Decision) -> ActKind {
    match value {
        Decision::Accepted => ActKind::Accept,
        Decision::Rejected { .. } => ActKind::Reject,
        Decision::PassedUp => ActKind::PassUp,
        Decision::Released { .. } => ActKind::Release,
        Decision::LeftHeld { .. } => ActKind::LeaveHeld,
    }
}
fn decisions(ask: &Ask) -> Vec<(ObjectKey, ActKind)> {
    match ask {
        Ask::Release { task, .. } => vec![(ObjectKey::Task(*task), ActKind::Release)],
        Ask::Cancel { task, .. } => vec![(ObjectKey::Task(*task), ActKind::Cancel)],
        Ask::Move { task, .. } => vec![(ObjectKey::Task(*task), ActKind::Move)],
        Ask::Amend { task, .. } | Ask::Propose { action: ProposeAction::Amendment { task, .. }, .. } => {
            vec![(ObjectKey::Task(*task), ActKind::Amend)]
        }
        Ask::Stop { task, .. } => vec![(ObjectKey::Task(*task), ActKind::Stop)],
        Ask::Decide { on, .. } => match on {
            DecideOn::Proposal { proposal, decision: value, .. } => {
                vec![(ObjectKey::Proposal(*proposal), decision(value))]
            }
            DecideOn::Escalation { task, decision: value, .. } => vec![(ObjectKey::Escalation(*task), decision(value))],
        },
        Ask::Answer { on: AnswerOn::Person { task, .. }, .. } => vec![(ObjectKey::PersonTask(*task), ActKind::Answer)],
        Ask::DeleteNote { project, note, .. } => {
            vec![(ObjectKey::Note { project: *project, note: *note }, ActKind::DeleteNote)]
        }
        Ask::Compound { operations, .. } => operations
            .iter()
            .filter_map(|operation| match operation {
                Operation::Release { task, .. } => Some((ObjectKey::Task(*task), ActKind::Release)),
                Operation::Cancel { task, .. } => Some((ObjectKey::Task(*task), ActKind::Cancel)),
                Operation::Move { task, .. } => Some((ObjectKey::Task(*task), ActKind::Move)),
                Operation::Amend { task, .. } => Some((ObjectKey::Task(*task), ActKind::Amend)),
                Operation::Stop { task, .. } => Some((ObjectKey::Task(*task), ActKind::Stop)),
                Operation::Message { .. } => None,
            })
            .collect(),
        Ask::Answer { on: AnswerOn::Question { .. }, .. }
        | Ask::Propose { action: ProposeAction::Settings(_), .. }
        | Ask::Propose { action: ProposeAction::Batch { .. }, .. }
        | Ask::Withdraw { .. }
        | Ask::Create { .. }
        | Ask::Message { .. }
        | Ask::Take { .. }
        | Ask::HandBack { .. }
        | Ask::Seen { .. }
        | Ask::EditNote { .. }
        | Ask::ChangePolicy { .. }
        | Ask::SetRoles { .. }
        | Ask::MakeService { .. }
        | Ask::SignIn { .. }
        | Ask::AccountChange { .. }
        | Ask::SignOut { .. }
        | Ask::Rename { .. }
        | Ask::ChangeSettings(_)
        | Ask::MoveToNewest(_)
        | Ask::Restore(_)
        | Ask::MakeProject { .. }
        | Ask::SetPool { .. } => Vec::new(),
    }
}
