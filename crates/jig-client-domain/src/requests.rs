//! Keyed requests, memory-only Seen and kept work (client/domain/client.md, 6 and 11).
use crate::{
    Ask, Domain, Down, Held, Key, Limits, Outcome, Pending, Progress, Request, Retry, State, Told, contract,
    domain::{down, tell},
};
use skein_lib::{
    Env, List, Queue, RequestAdmission, RequestAnswered, RequestEnvelope, RequestKey, RequestOut, RequestProgress,
    RequestRecord, RequestRefused, Token,
};
#[derive(Debug)]
pub(crate) struct PendingRequest {
    request: Ask,
    pending: Pending,
    attempt: Option<Token>,
    admission: RequestAdmission,
}
impl Domain {
    #[must_use]
    pub fn pending(&self, key: Key) -> Option<Pending> {
        if let Some(carrying) = self.carried.get(&key) {
            return Some(carrying.pending());
        }
        let mut pending = self.requests.get(&key)?.pending;
        for owner in &self.owners {
            if Some(owner.spoken.family) == pending.owner && !owner.served {
                pending.progress = Progress::NotServed;
            }
        }
        Some(pending)
    }
    pub(crate) fn identity(&self) -> Option<(u64, skein_lib::Duration, bool)> {
        for (_, w) in &self.watches {
            if w.own {
                let Some(crate::Latest::Jig(contract::Delivery::Me(frame))) = &w.latest else {
                    continue;
                };
                return Some((
                    frame.party.number,
                    frame.retention,
                    w.state == State::Live && self.link == crate::Link::Live,
                ));
            }
        }
        None
    }
    pub(crate) fn send(
        &mut self,
        env: &Env<Limits>,
        key: Key,
        name: Option<RequestKey>,
        request: Ask,
        out: &mut Queue<Request>,
    ) {
        if let Ask::Family { record, .. } = &request {
            self.family(record);
        }
        assert!(!self.carried.contains_key(&key), "request key conflict");
        if let Some(held) = self.requests.get(&key) {
            let named = match name {
                None => true,
                Some(name) => name == held.admission.key,
            };
            assert!(held.request == request && named, "request key conflict");
            return;
        }
        let Some((party, _, _)) = self.identity() else {
            tell(out, Told::Answered { key, outcome: Outcome::Unconfirmed });
            return;
        };
        if let Some(name) = name
            && self.carrying(name)
        {
            tell(out, Told::Answered { key, outcome: Outcome::Held });
            return;
        }
        let size = declared(&env.limits, &request);
        let pending = pending(party, &request);
        let held = Held { key, request: request.clone() };
        let admission = match name {
            None => self.table.ask(key, party, size, held),
            Some(name) => self.table.ask_given(key, party, size, name, held),
        };
        match admission {
            Ok(admission) => {
                self.requests
                    .insert(key, PendingRequest { request, pending, attempt: None, admission })
                    .expect("table admitted request");
            }
            Err(RequestRefused::Full(_)) => tell(out, Told::Answered { key, outcome: Outcome::Limit }),
            Err(RequestRefused::Held(_)) => tell(out, Told::Answered { key, outcome: Outcome::Held }),
        }
    }
    /// Restores decoded records from the small store (client/domain/client.md, 11).
    pub fn restore_record(&mut self, env: &Env<Limits>, kept: List<RequestRecord<Held>>, out: &mut Queue<Request>) {
        let mut dropped = 0_u32;
        for record in kept.into_boxed() {
            let key = record.request.key;
            let request = record.request.request.clone();
            let pending = pending(record.scope, &request);
            let invalid = match &request {
                Ask::Family { record, .. } => !self.declared(record),
                Ask::Jig(_) => false,
            };
            let admission = if invalid || self.requests.contains_key(&key) || self.carried.contains_key(&key) {
                None
            } else {
                self.table.restore(key, declared(&env.limits, &request), record).ok()
            };
            if let Some(admission) = admission {
                self.requests
                    .insert(key, PendingRequest { request: request.clone(), pending, attempt: None, admission })
                    .expect("table admitted record");
                tell(out, Told::Kept { key, request });
            } else {
                dropped = dropped.checked_add(1).expect("restored list bounded");
            }
        }
        if dropped != 0 {
            tell(out, Told::Dropped { count: dropped });
        }
    }
    /// Retires a request whose last small-store write failed, in any table state.
    pub(crate) fn not_kept(&mut self, key: RequestKey, out: &mut Queue<Request>) {
        let Some(owner) = self.table.owner(key) else {
            return;
        };
        let Some(held) = self.requests.get(&owner) else {
            return;
        };
        if let Some(owner) = self.table.not_kept(key, held.admission.number) {
            self.requests.remove(&owner);
            tell(out, Told::Answered { key: owner, outcome: Outcome::NotKept });
        }
    }
    pub(crate) fn answer(
        &mut self,
        env: &Env<Limits>,
        attempt: Token,
        envelope: RequestEnvelope,
        answer: Option<crate::Answered>,
        why: Option<Retry>,
        out: &mut Queue<Request>,
    ) {
        let envelope =
            if answer.is_none() && envelope == RequestEnvelope::Final { RequestEnvelope::Lost } else { envelope };
        if let Some(key) = self.carried_by(attempt) {
            self.carried_answered(key, envelope, answer, out);
            return;
        }
        match self.table.answered(env.now, attempt, envelope) {
            RequestAnswered::Final(key) => {
                self.requests.remove(&key);
                tell(out, Told::Answered { key, outcome: Outcome::Answered(answer.expect("decoded final answer")) });
            }
            RequestAnswered::Pending => {
                let mut found = None;
                for (key, held) in &self.requests {
                    if held.attempt == Some(attempt) {
                        found = Some(*key);
                    }
                }
                if let Some(key) = found {
                    let held = self.requests.get_mut(&key).expect("attempt request");
                    held.pending.progress = Progress::Retrying { why: why.unwrap_or(Retry::Lost) };
                }
            }
            RequestAnswered::Stale => {}
        }
    }
    pub(crate) fn flush(&mut self, env: &Env<Limits>, out: &mut Queue<Request>) {
        match self.identity() {
            Some((party, retention, live)) => {
                self.table.confirmed(Some(party));
                self.table.retention(retention);
                self.table.link(live);
            }
            None => {
                self.table.link(false);
                self.table.confirmed(None);
            }
        }
        {
            let mut cursor = self.table.take(env.now, env.wall);
            for _ in 0..env.limits.requests.out {
                match cursor.next_out() {
                    Some(RequestOut::Save(record)) => {
                        if kept(&record.request.request) {
                            down(out, Down::Save { record: record.clone() });
                        }
                    }
                    Some(RequestOut::Erase(key)) => down(out, Down::Erase { key }),
                    Some(RequestOut::Send { attempt, key, request }) => {
                        self.requests.get_mut(&request.key).expect("sending request").attempt = Some(attempt);
                        let mut refused = false;
                        if let Ask::Family { record, .. } = &request.request {
                            for owner in &self.owners {
                                if owner.spoken.family == record.family && !owner.served {
                                    refused = true;
                                }
                            }
                        }
                        if refused {
                            self.blocked.push(attempt).expect("one attempt per request");
                            self.requests.get_mut(&request.key).expect("sending request").pending.progress =
                                Progress::NotServed;
                        } else {
                            down(out, Down::Send { attempt, key, request: request.request.clone(), value: None });
                        }
                    }
                    None => break,
                }
            }
        }
        for attempt in &self.blocked {
            match self.table.answered(env.now, *attempt, RequestEnvelope::Again) {
                RequestAnswered::Pending => {}
                RequestAnswered::Final(_) | RequestAnswered::Stale => unreachable!("current refused attempt"),
            }
        }
        self.blocked.clear();
        for _ in 0..env.limits.requests.requests {
            let Some((key, progress)) = self.table.progress() else {
                break;
            };
            let held = self.requests.get_mut(&key).expect("progress names held request");
            held.pending.progress = match progress {
                RequestProgress::InFlight => Progress::InFlight,
                RequestProgress::Parked => Progress::Parked,
                RequestProgress::Unknown => Progress::Unknown,
                RequestProgress::Retrying => match held.pending.progress {
                    Progress::NotServed => Progress::NotServed,
                    Progress::Retrying { why } => Progress::Retrying { why },
                    Progress::InFlight | Progress::Parked | Progress::Unknown => {
                        Progress::Retrying { why: Retry::Lost }
                    }
                },
            };
            if progress == RequestProgress::Unknown {
                self.requests.remove(&key);
                tell(out, Told::Answered { key, outcome: Outcome::Unknown });
            } else {
                tell(out, Told::Pending { key });
            }
        }
    }
}
fn kept(request: &Ask) -> bool {
    match request {
        Ask::Jig(request) => match request.as_ref() {
            contract::Ask::Seen { .. } => false,
            contract::Ask::ChangeSettings(_)
            | contract::Ask::MoveToNewest(_)
            | contract::Ask::Restore(_)
            | contract::Ask::Create { .. }
            | contract::Ask::Propose { .. }
            | contract::Ask::Withdraw { .. }
            | contract::Ask::Message { .. }
            | contract::Ask::Compound { .. }
            | contract::Ask::Answer { .. }
            | contract::Ask::Take { .. }
            | contract::Ask::HandBack { .. }
            | contract::Ask::Decide { .. }
            | contract::Ask::Release { .. }
            | contract::Ask::Amend { .. }
            | contract::Ask::Stop { .. }
            | contract::Ask::Cancel { .. }
            | contract::Ask::Move { .. }
            | contract::Ask::EditNote { .. }
            | contract::Ask::DeleteNote { .. }
            | contract::Ask::ChangePolicy { .. }
            | contract::Ask::SetRoles { .. }
            | contract::Ask::MakeService { .. }
            | contract::Ask::SetPool { .. }
            | contract::Ask::Rename { .. }
            | contract::Ask::MakeProject { .. }
            | contract::Ask::SignIn { .. }
            | contract::Ask::AccountChange { .. }
            | contract::Ask::SignOut { .. } => true,
        },
        Ask::Family { .. } => true,
    }
}
pub(crate) fn declared(limits: &Limits, request: &Ask) -> u64 {
    match request {
        Ask::Jig(_) => limits.ask_bytes,
        Ask::Family { record: f, .. } => u64::try_from(f.bytes.len()).expect("family bytes fit"),
    }
}
pub(crate) fn pending(party: u64, request: &Ask) -> Pending {
    let owner = match request {
        Ask::Jig(_) => None,
        Ask::Family { record: f, .. } => Some(f.family),
    };
    Pending { party, owner, progress: Progress::Parked }
}
