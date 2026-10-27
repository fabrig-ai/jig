//! Requests are decided once in the party's key space and released after durability.
//! Revision and decided-first checks precede scripted outcomes; contract.md, 3–4.
mod creating;
mod priorities;
mod projects;
use crate::{Engine, Envelope, Invalid, Keyed, RequestIdentity, Retained, Up, payload};
use jig_client_domain::contract::{
    ActKind, Actor, Admission, Answer, Ask, ChipPhase, CurrentHold, Decision, Ending, Moved, ObjectKey, Offered,
    Outcome, OwnerRequester, Phase, ReadValue, Recipient, Refusal, Revision, Runtime, TaskHold,
};
use jig_client_domain::contract::{AnswerOn, DecideOn, Operation};
use skein_lib::{List, Time, Token, Wall};

impl<A: crate::Application> Engine<A> {
    /// Runs one attempt; the world's root supplies the table's attempt token.
    /// Invalid typed payloads are rejected before anything is decided or retained.
    pub fn send(
        &mut self,
        now: Time,
        wall: Wall,
        sign_in: u64,
        key: [u8; 16],
        attempt: Token,
        ask: Ask,
    ) -> core::result::Result<(), Invalid> {
        if !payload::fits(&ask, &self.bounds) {
            return Err(Invalid::Payload);
        }
        self.attempts.insert(attempt, sign_in);
        if !self.online(now) || self.request_fault == Some(Envelope::Again) {
            self.request_fault = None;
            self.queue(now, 0, Up::Answer { attempt, envelope: Envelope::Again, answer: None });
            return Ok(());
        }
        let Some(party) = self.truth.sign_ins.get(&sign_in).copied() else {
            self.queue(now, 0, Up::Answer { attempt, envelope: Envelope::SignedOut, answer: None });
            return Ok(());
        };
        self.keys.retain(|_, keyed| keyed.retained_until.is_none_or(|until| now <= until));
        if let Some(old) = self.keys.get(&(party, key)) {
            let answer = if let (RequestIdentity::Jig(previous), Retained::Jig(answer)) = (&old.ask, &old.answer) {
                if **previous == ask {
                    answer.clone()
                } else {
                    Answer::Refused { refusal: Refusal::KeyConflict, at: None }
                }
            } else {
                Answer::Refused { refusal: Refusal::KeyConflict, at: None }
            };
            let commit = old.commit;
            self.answer(now, commit, attempt, answer);
            return Ok(());
        }
        let (answer, changes) = match self.decide(party, &ask, wall) {
            Ok(decision) => decision,
            Err(error) => {
                self.attempts.remove(&attempt);
                return Err(error);
            }
        };
        let commit = self.commit(now, wall, Some(party), Some(key), Some(ask.clone()), Some(answer.clone()), changes);
        self.keys.insert(
            (party, key),
            Keyed {
                ask: RequestIdentity::Jig(Box::new(ask)),
                answer: Retained::Jig(answer.clone()),
                commit,
                retained_until: None,
            },
        );
        self.answer(now, commit, attempt, answer);
        Ok(())
    }
    fn answer(&mut self, now: Time, commit: u64, attempt: Token, answer: Answer) {
        let envelope = self.request_fault.take().unwrap_or(Envelope::Final);
        self.queue(
            now,
            commit,
            Up::Answer { attempt, envelope, answer: if envelope == Envelope::Final { Some(answer) } else { None } },
        );
    }
    #[expect(
        clippy::too_many_lines,
        reason = "one exhaustive decision checks all revision and offered-act variants before changing truth"
    )]
    pub(crate) fn decide(
        &mut self,
        party: u64,
        ask: &Ask,
        wall: Wall,
    ) -> core::result::Result<(Answer, Vec<crate::Change>), Invalid> {
        if let Ask::ChangeSettings(_) | Ask::MoveToNewest(_) = ask {
            let held = self.truth.frames.get(&party).and_then(|me| me.deployment_role);
            if held != Some(jig_client_domain::contract::HeldRole::Owner) {
                return Ok((Answer::Refused { refusal: Refusal::Role { held }, at: None }, Vec::new()));
            }
            if let Some(rule) = self
                .rules
                .iter()
                .find(|r| r.ask == *ask && r.party == Some(party))
                .or_else(|| self.rules.iter().find(|r| r.ask == *ask && r.party.is_none()))
                .cloned()
            {
                let mut next = self.truth.clone();
                for change in &rule.changes {
                    next.apply(change)?;
                }
                self.truth = next;
                return Ok((rule.answer, rule.changes));
            }
            return Ok((Answer::Refused { refusal: Refusal::NotOffered, at: None }, Vec::new()));
        }
        if let Some(result) = self.project_request(party, ask) {
            return result;
        }
        if let Ask::Compound { project, operations } = ask
            && priorities::only(operations)
        {
            return self.priorities(party, *project, operations, wall);
        }
        if let Ask::Withdraw { .. } = ask {
            return Ok((Answer::Refused { refusal: Refusal::NotOffered, at: None }, Vec::new()));
        }
        if let Ask::Compound { project, operations } = ask
            && compound_target(*project, operations).is_none()
        {
            return Ok((Answer::Refused { refusal: Refusal::NotOffered, at: None }, Vec::new()));
        }
        let Some((project, object, kind, revision)) = target(ask) else {
            return Ok((Answer::Refused { refusal: Refusal::NotOffered, at: None }, Vec::new()));
        };
        if project.is_some_and(|project| !self.truth.visible(party, project)) {
            return Ok((Answer::Refused { refusal: Refusal::Unknown, at: None }, Vec::new()));
        }
        if let Ask::EditNote { note, revision, .. } | Ask::DeleteNote { note, revision, .. } = ask {
            let Some(current) = self.truth.notes.get(note) else {
                return Ok((Answer::Refused { refusal: Refusal::Unknown, at: None }, Vec::new()));
            };
            if *revision != current.revision {
                return Ok((
                    Answer::Refused { refusal: Refusal::Moved { now: current.revision, what: Moved::Note }, at: None },
                    Vec::new(),
                ));
            }
        }
        if let Some(key) = object {
            if project != self.truth.project(key) {
                return Ok((Answer::Refused { refusal: Refusal::Unknown, at: None }, Vec::new()));
            }
            let Some(value) = self.truth.objects.get(&key) else {
                return Ok((Answer::Refused { refusal: Refusal::Unknown, at: None }, Vec::new()));
            };
            if let Some(Ending::Decided { party: Some(by), how: Some(how), at, .. }) = ending(value) {
                return Ok((Answer::Done(Outcome::DecidedFirst { by: by.clone(), how: how.clone(), at }), Vec::new()));
            }
            if let Some(how) = ending(value) {
                return Ok((Answer::Refused { refusal: Refusal::Ended { how: how.clone() }, at: None }, Vec::new()));
            }
            if let Some((seen, what)) = revision {
                let current = match (value, &what) {
                    (ReadValue::Task(value), Moved::Spec) => Some(value.spec_revision),
                    (ReadValue::OwnerEscalation(value), Moved::Spec) => Some(value.task.spec_revision),
                    (ReadValue::OwnerEscalation(value), Moved::Hold { .. }) => {
                        value.task.hold.as_ref().map(TaskHold::revision)
                    }
                    (ReadValue::Task(value), Moved::Hold { .. }) => value.hold.as_ref().map(TaskHold::revision),
                    (ReadValue::Escalation(value), Moved::Hold { .. }) => Some(value.hold.revision),
                    (ReadValue::PersonTask(value), Moved::PersonTask) => Some(value.revision),
                    (ReadValue::OwnerPersonTask(value), Moved::PersonTask) => Some(value.revision),
                    (
                        ReadValue::Tasks(_)
                        | ReadValue::Me(_)
                        | ReadValue::Task(_)
                        | ReadValue::Proposal(_)
                        | ReadValue::Historical(_)
                        | ReadValue::OwnerProposal(_)
                        | ReadValue::Escalation(_)
                        | ReadValue::OwnerEscalation(_)
                        | ReadValue::Question(_)
                        | ReadValue::PersonTask(_)
                        | ReadValue::OwnerPersonTask(_)
                        | ReadValue::Result(_)
                        | ReadValue::Ended(_)
                        | ReadValue::History(_)
                        | ReadValue::Activity(_)
                        | ReadValue::Turn(_)
                        | ReadValue::Inbox(_)
                        | ReadValue::OwnerInbox(_)
                        | ReadValue::Spend(_)
                        | ReadValue::OwnerEnded(_)
                        | ReadValue::OwnerSpend(_)
                        | ReadValue::OwnerProject(_)
                        | ReadValue::Project(_)
                        | ReadValue::Notes(_)
                        | ReadValue::Settings(_)
                        | ReadValue::Setting(_)
                        | ReadValue::Revision(_)
                        | ReadValue::Revisions(_)
                        | ReadValue::Deployment(_)
                        | ReadValue::OwnerDeployment(_),
                        Moved::Hold { .. }
                        | Moved::PlacedHold { .. }
                        | Moved::Spec
                        | Moved::PersonTask
                        | Moved::Note
                        | Moved::Waiting,
                    ) => None,
                };
                if let Some(now) = current {
                    if seen != now {
                        let what = match what {
                            Moved::Hold { .. } => {
                                let reason = match value {
                                    ReadValue::Task(task) => task.hold.as_ref().map(task_hold_reason),
                                    ReadValue::OwnerEscalation(value) => value.task.hold.as_ref().map(task_hold_reason),
                                    ReadValue::Escalation(escalation) => {
                                        Some(CurrentHold::Document(Box::new(escalation.hold.reason.clone())))
                                    }
                                    ReadValue::Tasks(_)
                                    | ReadValue::Me(_)
                                    | ReadValue::Proposal(_)
                                    | ReadValue::Historical(_)
                                    | ReadValue::OwnerProposal(_)
                                    | ReadValue::Question(_)
                                    | ReadValue::PersonTask(_)
                                    | ReadValue::OwnerPersonTask(_)
                                    | ReadValue::Result(_)
                                    | ReadValue::Ended(_)
                                    | ReadValue::Inbox(_)
                                    | ReadValue::OwnerInbox(_)
                                    | ReadValue::Activity(_)
                                    | ReadValue::Turn(_)
                                    | ReadValue::History(_)
                                    | ReadValue::Spend(_)
                                    | ReadValue::OwnerEnded(_)
                                    | ReadValue::OwnerSpend(_)
                                    | ReadValue::OwnerProject(_)
                                    | ReadValue::Project(_)
                                    | ReadValue::Notes(_)
                                    | ReadValue::Settings(_)
                                    | ReadValue::Setting(_)
                                    | ReadValue::Revision(_)
                                    | ReadValue::Revisions(_)
                                    | ReadValue::Deployment(_)
                                    | ReadValue::OwnerDeployment(_) => None,
                                };
                                Moved::Hold { reason }
                            }
                            Moved::PlacedHold { .. }
                            | Moved::Spec
                            | Moved::PersonTask
                            | Moved::Note
                            | Moved::Waiting => what,
                        };
                        return Ok((Answer::Refused { refusal: Refusal::Moved { now, what }, at: None }, Vec::new()));
                    }
                } else {
                    return Ok((Answer::Refused { refusal: Refusal::NotOffered, at: None }, Vec::new()));
                }
            }
            if let Ask::Compound { operations, .. } = ask
                && let Some(Operation::Release { revision: seen, .. }) = operations.as_slice().last()
                && let ReadValue::Task(task) = value
                && let Some(hold) = &task.hold
                && *seen != hold.revision()
            {
                return Ok((
                    Answer::Refused {
                        refusal: Refusal::Moved {
                            now: hold.revision(),
                            what: Moved::Hold { reason: Some(task_hold_reason(hold)) },
                        },
                        at: None,
                    },
                    Vec::new(),
                ));
            }
            let waiting = match value {
                ReadValue::Proposal(value) => Some(&value.waiting),
                ReadValue::Escalation(value) => Some(&value.waiting),
                ReadValue::Historical(_)
                | ReadValue::OwnerProposal(_)
                | ReadValue::OwnerEscalation(_)
                | ReadValue::Tasks(_)
                | ReadValue::Me(_)
                | ReadValue::Task(_)
                | ReadValue::Question(_)
                | ReadValue::PersonTask(_)
                | ReadValue::OwnerPersonTask(_)
                | ReadValue::Result(_)
                | ReadValue::Ended(_)
                | ReadValue::History(_)
                | ReadValue::Activity(_)
                | ReadValue::Turn(_)
                | ReadValue::Inbox(_)
                | ReadValue::OwnerInbox(_)
                | ReadValue::Spend(_)
                | ReadValue::OwnerEnded(_)
                | ReadValue::OwnerSpend(_)
                | ReadValue::OwnerProject(_)
                | ReadValue::Project(_)
                | ReadValue::Notes(_)
                | ReadValue::Settings(_)
                | ReadValue::Setting(_)
                | ReadValue::Revision(_)
                | ReadValue::Revisions(_)
                | ReadValue::Deployment(_)
                | ReadValue::OwnerDeployment(_) => None,
            };
            if waiting.is_some_and(|waiting| !recipient(&self.truth, party, waiting)) {
                return Ok((
                    Answer::Refused {
                        refusal: Refusal::Moved { now: Revision { value: 0 }, what: Moved::Waiting },
                        at: None,
                    },
                    Vec::new(),
                ));
            }
        }
        let note_offers = match ask {
            Ask::EditNote { note, .. } | Ask::DeleteNote { note, .. } => {
                self.truth.notes.get(note).map(|note| &note.acts)
            }
            Ask::Hold { .. }
            | Ask::Lift { .. }
            | Ask::Propose { .. }
            | Ask::Withdraw { .. }
            | Ask::Create { .. }
            | Ask::Compound { .. }
            | Ask::Message { .. }
            | Ask::Answer { .. }
            | Ask::Take { .. }
            | Ask::HandBack { .. }
            | Ask::Decide { .. }
            | Ask::Release { .. }
            | Ask::Amend { .. }
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
            | Ask::ChangeSettings(_)
            | Ask::MoveToNewest(_)
            | Ask::MakeProject { .. }
            | Ask::SetPool { .. } => None,
        };
        let offer = note_offers.or_else(|| {
            object
                .and_then(|key| self.truth.objects.get(&key))
                .and_then(offers)
                .or_else(|| project.and_then(|p| self.truth.owner_projects.get(&p).map(|p| &p.acts)))
                .or_else(|| project.and_then(|p| self.truth.projects.get(&p).map(|p| &p.acts)))
        });
        let creating = self.creating_admission(party, ask);
        let admission = match &creating {
            Some(value) => Some(value),
            None => {
                offer.and_then(|acts| acts.iter().find(|offered| offered.act == kind).map(|offered| &offered.admission))
            }
        };
        let bounded = matches!(admission, Some(Admission::Bounded { .. }));
        let prediction = admission.map(|offer| match offer {
            Admission::Bounded { .. } => match ask {
                Ask::Create { tasks, .. }
                | Ask::Propose { action: jig_client_domain::contract::ProposeAction::Batch { tasks }, .. } => {
                    Self::batch_admission(offer, tasks, wall)
                }
                Ask::Amend { task, change, .. }
                | Ask::Propose {
                    action: jig_client_domain::contract::ProposeAction::Amendment { task, change, .. },
                    ..
                } => self.amendment_admission(offer, *task, change),
                Ask::Compound { operations, .. } => {
                    let [Operation::Amend { task, change, .. }, Operation::Release { .. }] = operations.as_slice()
                    else {
                        return Admission::Refused(Refusal::NotOffered);
                    };
                    self.amendment_admission(offer, *task, change)
                }
                Ask::Propose { action: jig_client_domain::contract::ProposeAction::Settings(_), .. }
                | Ask::Message { .. }
                | Ask::Answer { .. }
                | Ask::Take { .. }
                | Ask::HandBack { .. }
                | Ask::Decide { .. }
                | Ask::Hold { .. }
                | Ask::Lift { .. }
                | Ask::Release { .. }
                | Ask::Stop { .. }
                | Ask::Cancel { .. }
                | Ask::Move { .. }
                | Ask::Seen { .. }
                | Ask::EditNote { .. }
                | Ask::SignIn { .. }
                | Ask::AccountChange { .. }
                | Ask::SignOut { .. }
                | Ask::DeleteNote { .. }
                | Ask::ChangePolicy { .. }
                | Ask::SetRoles { .. }
                | Ask::MakeService { .. }
                | Ask::Withdraw { .. }
                | Ask::Rename { .. }
                | Ask::ChangeSettings(_)
                | Ask::MoveToNewest(_)
                | Ask::MakeProject { .. }
                | Ask::SetPool { .. } => Admission::Refused(Refusal::NotOffered),
            },
            Admission::Allowed | Admission::Proposed { .. } | Admission::Refused(_) => offer.clone(),
        });
        let prediction = if prediction == Some(Admission::Allowed) {
            self.accepting_admission(party, ask, wall).or(prediction)
        } else {
            prediction
        };
        let admission = prediction.as_ref();
        if let Some(Admission::Refused(refusal)) = admission {
            return Ok((Answer::Refused { refusal: refusal.clone(), at: None }, Vec::new()));
        }
        if let Some(rule) = self
            .rules
            .iter()
            .find(|rule| rule.ask == *ask && rule.party == Some(party))
            .or_else(|| self.rules.iter().find(|rule| rule.ask == *ask && rule.party.is_none()))
            .cloned()
        {
            if bounded && matches!(rule.answer, Answer::Refused { refusal: _, .. }) {
                return Err(Invalid::Script);
            }
            if matches!(rule.answer, Answer::Done(_)) {
                if (matches!(admission, Some(Admission::Proposed { .. })) || matches!(ask, Ask::Propose { .. }))
                    && !matches!(
                        rule.answer,
                        Answer::Done(
                            Outcome::Proposed { .. }
                                | Outcome::Owner(jig_client_domain::contract::OwnerOutcome::Proposed { .. })
                        )
                    )
                {
                    return Err(Invalid::Script);
                }
                if bounded
                    && !matches!(ask, Ask::Propose { .. })
                    && matches!(admission, Some(Admission::Allowed))
                    && matches!(
                        rule.answer,
                        Answer::Done(
                            Outcome::Proposed { .. }
                                | Outcome::Owner(jig_client_domain::contract::OwnerOutcome::Proposed { .. })
                        )
                    )
                {
                    return Err(Invalid::Script);
                }
                if admission.is_none() && !matches!(ask, Ask::Seen { .. }) {
                    return Ok((Answer::Refused { refusal: Refusal::NotOffered, at: None }, Vec::new()));
                }
            }
            let mut next = self.truth.clone();
            for change in &rule.changes {
                next.apply(change)?;
            }
            self.truth = next;
            return Ok((rule.answer, rule.changes));
        }
        if admission.is_none() {
            return Ok((Answer::Refused { refusal: Refusal::NotOffered, at: None }, Vec::new()));
        }
        if let Some(Admission::Proposed { .. }) = admission {
            return Ok((Answer::Refused { refusal: Refusal::NotOffered, at: None }, Vec::new()));
        }
        let how = match ask {
            Ask::Decide { on: DecideOn::Proposal { decision, .. }, .. }
            | Ask::Decide { on: DecideOn::Escalation { decision, .. }, .. } => Some(decision.clone()),
            Ask::Release { again, .. } => Some(Decision::Released { again: *again }),
            Ask::Hold { .. }
            | Ask::Lift { .. }
            | Ask::Propose { .. }
            | Ask::Withdraw { .. }
            | Ask::Create { .. }
            | Ask::Compound { .. }
            | Ask::Message { .. }
            | Ask::Answer { .. }
            | Ask::Take { .. }
            | Ask::HandBack { .. }
            | Ask::Amend { .. }
            | Ask::Stop { .. }
            | Ask::Cancel { .. }
            | Ask::Move { .. }
            | Ask::Seen { .. }
            | Ask::EditNote { .. }
            | Ask::SignIn { .. }
            | Ask::AccountChange { .. }
            | Ask::SignOut { .. }
            | Ask::DeleteNote { .. }
            | Ask::ChangePolicy { .. }
            | Ask::SetRoles { .. }
            | Ask::MakeService { .. }
            | Ask::Rename { .. }
            | Ask::ChangeSettings(_)
            | Ask::MoveToNewest(_)
            | Ask::MakeProject { .. }
            | Ask::SetPool { .. } => None,
        };
        if let (Some(key), Some(how)) = (object, how) {
            if matches!(how, Decision::PassedUp | Decision::LeftHeld { .. }) {
                return Ok((Answer::Refused { refusal: Refusal::NotOffered, at: None }, Vec::new()));
            }
            let by = self.truth.frames.get(&party).expect("signed-in party has a frame").party.clone();
            let mut value = self.truth.objects.get(&key).expect("target checked").clone();
            // Terminal Party labels have no loss carrier; retain the exact actor
            // number and only a complete supplied optional name. Me keeps its cut.
            let label = by.name.whole().map(|name| jig_client_domain::contract::Party {
                number: by.number,
                kind: by.kind,
                name,
            });
            let terminal = Ending::Decided { by: by.number, party: label, how: Some(how.clone()), at: wall };
            match &mut value {
                ReadValue::Proposal(v) => v.ending = Some(terminal),
                ReadValue::Escalation(v) => v.ending = Some(terminal),
                ReadValue::Task(v) => {
                    v.hold = None;
                    if let Some(context) = &mut v.context {
                        context.escalation = None;
                        context.runtime = Runtime::Starting;
                    }
                    v.chip.phase = Phase::Due;
                    *v.chip.details = ChipPhase::Starting;
                }
                ReadValue::Historical(_)
                | ReadValue::OwnerProposal(_)
                | ReadValue::OwnerEscalation(_)
                | ReadValue::Tasks(_)
                | ReadValue::Me(_)
                | ReadValue::Question(_)
                | ReadValue::PersonTask(_)
                | ReadValue::OwnerPersonTask(_)
                | ReadValue::Result(_)
                | ReadValue::Ended(_)
                | ReadValue::History(_)
                | ReadValue::Activity(_)
                | ReadValue::Turn(_)
                | ReadValue::Inbox(_)
                | ReadValue::OwnerInbox(_)
                | ReadValue::Spend(_)
                | ReadValue::OwnerEnded(_)
                | ReadValue::OwnerSpend(_)
                | ReadValue::OwnerProject(_)
                | ReadValue::Project(_)
                | ReadValue::Notes(_)
                | ReadValue::Settings(_)
                | ReadValue::Setting(_)
                | ReadValue::Revision(_)
                | ReadValue::Revisions(_)
                | ReadValue::Deployment(_)
                | ReadValue::OwnerDeployment(_) => {
                    return Ok((Answer::Refused { refusal: Refusal::NotOffered, at: None }, Vec::new()));
                }
            }
            let change =
                crate::Change::Object { project: project.expect("object requests name a project"), key, value };
            change.validate(&self.bounds)?;
            self.truth.apply(&change)?;
            return Ok((Answer::Done(Outcome::Decided { by: Actor::Party(party), how, at: wall }), vec![change]));
        }
        Ok((Answer::Refused { refusal: Refusal::NotOffered, at: None }, Vec::new()))
    }
    fn amendment_admission(
        &self,
        offer: &Admission,
        task: u64,
        change: &jig_client_domain::contract::Amendment,
    ) -> Admission {
        match self.truth.objects.get(&ObjectKey::Task(task)) {
            Some(ReadValue::Task(before)) => {
                jig_client_domain::contract::admission::amendment(offer, change, &before.authority)
            }
            _ => match self.truth.objects.get(&ObjectKey::Escalation(task)) {
                Some(ReadValue::OwnerEscalation(before)) => {
                    jig_client_domain::contract::admission::amendment(offer, change, &before.task.authority)
                }
                Some(ReadValue::Escalation(before)) => {
                    jig_client_domain::contract::admission::amendment(offer, change, &before.steering.authority)
                }
                _ => Admission::Refused(Refusal::Unknown),
            },
        }
    }
}
pub(crate) fn recipient(truth: &crate::Truth, party: u64, to: &Recipient) -> bool {
    match to {
        Recipient::Party(number) => *number == party,
        Recipient::Role { project, role } => truth.frames.get(&party).is_some_and(|frame| {
            frame.projects.iter().any(|p| {
                p.project == *project && p.role == jig_client_domain::contract::HeldRole::Policy { role: *role }
            })
        }),
        Recipient::Roles { project, roles } => truth.frames.get(&party).is_some_and(|frame| {
            frame.projects.iter().any(|p| {
                p.project == *project
                    && roles.iter().any(|role| p.role == jig_client_domain::contract::HeldRole::Policy { role: *role })
            })
        }),
        Recipient::Task(task) => truth.objects.get(&ObjectKey::Task(*task)).is_some_and(|value| {
            if let ReadValue::Task(v) = value { v.requester == OwnerRequester::Person(party) } else { false }
        }),
    }
}
fn offers(value: &ReadValue) -> Option<&List<Offered>> {
    match value {
        ReadValue::Task(v) => Some(&v.acts),
        ReadValue::Proposal(v) => Some(&v.acts),
        ReadValue::OwnerProposal(v) => Some(&v.acts),
        ReadValue::Escalation(v) => Some(&v.acts),
        ReadValue::OwnerEscalation(v) => Some(&v.acts),
        ReadValue::Question(v) => Some(&v.acts),
        ReadValue::PersonTask(v) => Some(&v.acts),
        ReadValue::OwnerPersonTask(v) => Some(&v.acts),
        ReadValue::Result(v) => Some(&v.acts),
        ReadValue::Historical(_)
        | ReadValue::Tasks(_)
        | ReadValue::Me(_)
        | ReadValue::Ended(_)
        | ReadValue::History(_)
        | ReadValue::Activity(_)
        | ReadValue::Turn(_)
        | ReadValue::Inbox(_)
        | ReadValue::OwnerInbox(_)
        | ReadValue::Spend(_)
        | ReadValue::OwnerEnded(_)
        | ReadValue::OwnerSpend(_)
        | ReadValue::OwnerProject(_)
        | ReadValue::Project(_)
        | ReadValue::Notes(_)
        | ReadValue::Settings(_)
        | ReadValue::Setting(_)
        | ReadValue::Revision(_)
        | ReadValue::Revisions(_)
        | ReadValue::Deployment(_)
        | ReadValue::OwnerDeployment(_) => None,
    }
}
fn task_hold_reason(value: &TaskHold) -> CurrentHold {
    match value {
        TaskHold::Document(value) => CurrentHold::Document(Box::new(value.reason.clone())),
        TaskHold::Owner(value) => CurrentHold::Reason(value.reason),
    }
}
fn ending(value: &ReadValue) -> Option<Ending> {
    match value {
        ReadValue::Task(v) => match v.result.as_ref()? {
            jig_client_domain::contract::TaskResult::Document(result) => Some(result.ending.clone()),
            jig_client_domain::contract::TaskResult::Ending(ending) => Some(Ending::Task(ending.clone())),
        },
        ReadValue::Proposal(v) => v.ending.clone(),
        ReadValue::Escalation(v) => v.ending.clone(),
        ReadValue::Question(v) => v.ending.clone(),
        ReadValue::PersonTask(v) => v.ending.clone(),
        ReadValue::Result(v) => Some(v.ending.clone()),
        ReadValue::OwnerPersonTask(_)
        | ReadValue::Historical(_)
        | ReadValue::OwnerProposal(_)
        | ReadValue::OwnerEscalation(_)
        | ReadValue::Tasks(_)
        | ReadValue::Me(_)
        | ReadValue::Ended(_)
        | ReadValue::History(_)
        | ReadValue::Activity(_)
        | ReadValue::Turn(_)
        | ReadValue::Inbox(_)
        | ReadValue::OwnerInbox(_)
        | ReadValue::Spend(_)
        | ReadValue::OwnerEnded(_)
        | ReadValue::OwnerSpend(_)
        | ReadValue::OwnerProject(_)
        | ReadValue::Project(_)
        | ReadValue::Notes(_)
        | ReadValue::Settings(_)
        | ReadValue::Setting(_)
        | ReadValue::Revision(_)
        | ReadValue::Revisions(_)
        | ReadValue::Deployment(_)
        | ReadValue::OwnerDeployment(_) => None,
    }
}
fn decision_kind(decision: &Decision) -> ActKind {
    match decision {
        Decision::Accepted => ActKind::Accept,
        Decision::Rejected { .. } => ActKind::Reject,
        Decision::PassedUp => ActKind::PassUp,
        Decision::Released { .. } => ActKind::Release,
        Decision::LeftHeld { .. } => ActKind::LeaveHeld,
    }
}
type Target = (Option<u32>, Option<ObjectKey>, ActKind, Option<(Revision, Moved)>);
fn target(ask: &Ask) -> Option<Target> {
    Some(match ask {
        Ask::Hold { .. }
        | Ask::Lift { .. }
        | Ask::ChangeSettings(_)
        | Ask::MoveToNewest(_)
        | Ask::MakeProject { .. }
        | Ask::SignIn { .. }
        | Ask::SignOut { .. }
        | Ask::AccountChange { .. } => return None,
        Ask::Compound { project, operations } => {
            compound_target(*project, operations).unwrap_or((Some(*project), None, ActKind::Message, None))
        }
        Ask::Create { project, .. } => (Some(*project), None, ActKind::Message, None),
        Ask::Message { project, task, .. } => (Some(*project), Some(ObjectKey::Task(*task)), ActKind::Message, None),
        Ask::Answer { project, on: AnswerOn::Question { task, message, .. }, .. } => {
            (Some(*project), Some(ObjectKey::Question { task: *task, message: *message }), ActKind::Answer, None)
        }
        Ask::Answer { project, on: AnswerOn::Person { task, revision, .. }, .. } => {
            (Some(*project), Some(ObjectKey::PersonTask(*task)), ActKind::Answer, Some((*revision, Moved::PersonTask)))
        }
        Ask::Take { project, task } => (Some(*project), Some(ObjectKey::PersonTask(*task)), ActKind::Take, None),
        Ask::HandBack { project, task } => {
            (Some(*project), Some(ObjectKey::PersonTask(*task)), ActKind::HandBack, None)
        }
        Ask::Decide { project, on: DecideOn::Proposal { proposal, decision, .. } } => {
            (Some(*project), Some(ObjectKey::Proposal(*proposal)), decision_kind(decision), None)
        }
        Ask::Decide { project, on: DecideOn::Escalation { task, revision, decision } } => (
            Some(*project),
            Some(ObjectKey::Escalation(*task)),
            decision_kind(decision),
            Some((*revision, Moved::Hold { reason: None })),
        ),
        Ask::Release { project, task, revision, .. } => (
            Some(*project),
            Some(ObjectKey::Task(*task)),
            ActKind::Release,
            Some((*revision, Moved::Hold { reason: None })),
        ),
        Ask::Propose { project, action, .. } => match action {
            jig_client_domain::contract::ProposeAction::Amendment { task, revision, .. } => {
                (Some(*project), Some(ObjectKey::Task(*task)), ActKind::Amend, Some((*revision, Moved::Spec)))
            }
            jig_client_domain::contract::ProposeAction::Settings(_)
            | jig_client_domain::contract::ProposeAction::Batch { .. } => {
                (Some(*project), None, ActKind::Message, None)
            }
        },
        Ask::Amend { project, task, revision, .. } => {
            (Some(*project), Some(ObjectKey::Task(*task)), ActKind::Amend, revision.map(|value| (value, Moved::Spec)))
        }
        Ask::Stop { project, task } => (Some(*project), Some(ObjectKey::Task(*task)), ActKind::Stop, None),
        Ask::Cancel { project, task, .. } => (Some(*project), Some(ObjectKey::Task(*task)), ActKind::Cancel, None),
        Ask::Move { project, task, .. } => (Some(*project), Some(ObjectKey::Task(*task)), ActKind::Move, None),
        Ask::Withdraw { project, .. } => (Some(*project), None, ActKind::Withdraw, None),
        Ask::Seen { .. } => (None, None, ActKind::Seen, None),
        Ask::EditNote { project, .. } => (Some(*project), None, ActKind::EditNote, None),
        Ask::DeleteNote { project, .. } => (Some(*project), None, ActKind::DeleteNote, None),
        Ask::ChangePolicy { project, .. } => (Some(*project), None, ActKind::ChangePolicy, None),
        Ask::SetRoles { project, .. } => (Some(*project), None, ActKind::SetRoles, None),
        Ask::MakeService { project, .. } => (Some(*project), None, ActKind::MakeService, None),
        Ask::Rename { project, .. } => (Some(*project), None, ActKind::Rename, None),
        Ask::SetPool { project, .. } => (Some(*project), None, ActKind::SetPool, None),
    })
}

fn compound_target(project: u32, operations: &List<Operation>) -> Option<Target> {
    match operations.as_slice() {
        [Operation::Message { project: first, task, .. }, Operation::Release { project: last, task: released, .. }]
            if *first == project && *last == project && task == released =>
        {
            Some((Some(project), Some(ObjectKey::Task(*task)), ActKind::Message, None))
        }
        [
            Operation::Amend { project: first, task, revision, .. },
            Operation::Release { project: last, task: released, .. },
        ] if *first == project && *last == project && task == released => {
            Some((Some(project), Some(ObjectKey::Task(*task)), ActKind::Amend, revision.map(|v| (v, Moved::Spec))))
        }
        operations => {
            let Operation::Cancel { project: named, task, .. } = operations.last()? else { return None };
            if *named != project || !operations.iter().all(|op| matches!(op, Operation::Cancel { project: named, .. } | Operation::Move { project: named, .. } if *named == project)) { return None; }
            Some((Some(project), Some(ObjectKey::Task(*task)), ActKind::Cancel, None))
        }
    }
}
