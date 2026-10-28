//! Public thread inputs and chronological borrowed lines (client.md, 10).
use super::{Thread, Words, later, live_turn};
use crate::{
    Key,
    contract::{
        Activity, ActivityLine, ActivityPage, Answer, ArchivedTurn, Outcome, OwnerOutcome, Party, PartyKind,
        ProvisionalTurnName, Read, RunState, RunText, Task, Text, Turn, TurnName, TurnProblem, TurnRead,
    },
};
use alloc::boxed::Box;
use skein_lib::{List, Wall};

/// A thread line, oldest first. Other activity remains available whole.
#[derive(Debug, PartialEq, Eq)]
pub enum Line<'a> {
    Words { by: Option<&'a Party>, words: &'a [u8], state: Words, at: Option<Wall> },
    Turn { name: TurnName, turn: &'a Turn, at: Wall },
    Undecoded { name: TurnName, why: TurnProblem, at: Wall },
    Provisional { name: ProvisionalTurnName, text: &'a [u8], cut: bool },
    RunBegan { attempt: u64 },
    RunEnded { attempt: u64 },
    Activity { line: &'a ActivityLine },
    Left { lines: u32 },
}
impl Thread {
    /// A task document announces the committed activity position.
    pub fn document(&mut self, task: &Task) -> Option<Read> {
        if task.chip.task != self.task {
            return None;
        }
        self.moved(task.activity)
    }
    /// Takes the requested activity page and fences committed provisional turns.
    pub fn page(&mut self, page: ActivityPage) -> Option<Read> {
        let next = self.accept(page);
        self.prune_live();
        next
    }
    /// Keeps each provisional turn until its committed replacement arrives.
    pub fn run(&mut self, run: &RunState, text: Option<&RunText>) {
        if run.task != self.task {
            return;
        }
        if self.attempt != run.attempt {
            self.live.clear();
            self.attempt = run.attempt;
        }
        if run.attempt.is_none() {
            self.live.clear();
            return;
        }
        let Some(text) = text else {
            return;
        };
        if run.attempt != Some(text.attempt) {
            return;
        }
        if let Some(committed) = self.committed_turn
            && !later(text.turn.turn(), committed)
        {
            return;
        }
        let text_bound = self.limits.text.min(u32::try_from(self.limits.bytes).unwrap_or(u32::MAX));
        let mut found = false;
        for at in 0..self.live.len() {
            let old = self.live.get_mut(at).expect("bounded live turn exists");
            if old.name.turn() == text.turn.turn() {
                if old.name.retry <= text.turn.retry {
                    *old = live_turn(text.clone(), text_bound);
                }
                found = true;
                break;
            }
        }
        if !found {
            if self.live.len() == self.live.capacity() {
                self.left = self.left.saturating_add(1);
                let mut retained = List::with_capacity(self.live.capacity());
                let old = core::mem::replace(&mut self.live, List::with_capacity(0));
                for (at, turn) in old.into_boxed().into_iter().enumerate() {
                    if at > 0 {
                        retained.push(turn).expect("removal fits live room");
                    }
                }
                self.live = retained;
            }
            match self.live.push(live_turn(text.clone(), text_bound)) {
                Ok(()) | Err(_) => {}
            }
        }
        self.prune_live();
    }
    pub(super) fn prune_live(&mut self) {
        let mut bytes = 0_u64;
        let mut from = self.live.len();
        let mut full = false;
        for offset in 0..self.live.len() {
            let at = self.live.len().saturating_sub(offset).saturating_sub(1);
            let turn = self.live.get(at).expect("reverse live turn exists");
            if let Some(name) = self.committed_turn
                && !later(turn.name.turn(), name)
            {
                continue;
            }
            let length = u64::try_from(turn.text.bytes().len()).expect("text length fits u64");
            if !full && bytes.saturating_add(length) <= self.limits.bytes {
                bytes = bytes.saturating_add(length);
                from = at;
            } else {
                full = true;
                self.left = self.left.saturating_add(1);
            }
        }
        let mut live = List::with_capacity(self.live.capacity());
        let old = core::mem::replace(&mut self.live, List::with_capacity(0));
        for (at, turn) in old.into_boxed().into_iter().enumerate() {
            if u32::try_from(at).expect("bounded live index fits") < from {
                continue;
            }
            if let Some(name) = self.committed_turn
                && !later(turn.name.turn(), name)
            {
                continue;
            }
            live.push(turn).expect("retained live turns fit");
        }
        self.live = live;
    }
    /// Keeps a pending message under its public request key and compound place.
    pub fn sending(&mut self, key: Key, by: u64, words: Text, place: u32) -> bool {
        let author =
            Party { number: by, kind: PartyKind::Person, name: Text::new(Box::from([]), 0).expect("empty name fits") };
        if !words.fits(self.limits.text) || !self.words.begin(key, Some(author), words) {
            return false;
        }
        for at in 0..self.words.rows.len() {
            let row = self.words.rows.get_mut(at).expect("word row exists");
            if row.request == Some(key) {
                row.place = place;
                break;
            }
        }
        true
    }
    /// A durable answer numbers each message by its operation's place.
    pub fn answered(&mut self, key: Key, answer: &Answer) {
        let mut place = None;
        for row in &self.words.rows {
            if row.request == Some(key) {
                place = Some(row.place);
                break;
            }
        }
        let Some(place) = place else {
            return;
        };
        let message = match answer {
            Answer::Done(Outcome::Owner(value)) => number(value, self.task, place),
            Answer::Done(
                Outcome::Made { .. }
                | Outcome::Proposed { .. }
                | Outcome::Decided { .. }
                | Outcome::DecidedFirst { .. }
                | Outcome::Changed
                | Outcome::SettingsChanged(_)
                | Outcome::MovedToNewest(_),
            )
            | Answer::Refused { .. } => None,
        };
        match message {
            Some(message) => {
                let applied = self.words.sent(key, message);
                assert!(applied, "pending words exist");
            }
            None => self.words.refused(key),
        }
    }
    /// Borrows committed activity, pending words and provisional turns in order.
    #[must_use]
    pub fn lines(&self) -> List<Line<'_>> {
        let mut lines = List::with_capacity(self.limits.lines.saturating_mul(3));
        for offset in 0..self.lines.rows.len() {
            let at = self.lines.rows.len().saturating_sub(offset).saturating_sub(1);
            let row = self.lines.rows.get(at).expect("reverse row exists");
            let line = match &row.event {
                Activity::Words { by, message, words } => Line::Words {
                    by: Some(by),
                    words: words.bytes(),
                    state: self.words.state(*message),
                    at: Some(row.at),
                },
                Activity::Turn { name, turn }
                | Activity::Owner { decoded: Some(ArchivedTurn { name, turn, .. }), .. } => match turn {
                    TurnRead::Decoded(turn) => Line::Turn { name: *name, turn, at: row.at },
                    TurnRead::Undecodable { why } => Line::Undecoded { name: *name, why: *why, at: row.at },
                },
                Activity::RunBegan { attempt, .. } => Line::RunBegan { attempt: *attempt },
                Activity::RunEnded { attempt, .. } => Line::RunEnded { attempt: *attempt },
                Activity::Owner { decoded: None, .. }
                | Activity::Result { .. }
                | Activity::News { .. }
                | Activity::Decision { .. } => Line::Activity { line: row },
            };
            lines.push(line).expect("thread projection fits its three bounded sets");
        }
        for row in &self.words.rows {
            let mut held = false;
            for line in &self.lines.rows {
                if let Some((message, _)) = line.event.word_message()
                    && self.words.state(message) == row.state
                {
                    held = true;
                    break;
                }
            }
            if !held {
                lines
                    .push(Line::Words { by: row.by.as_ref(), words: row.words.bytes(), state: row.state, at: None })
                    .expect("word projection fits");
            }
        }
        for turn in &self.live {
            lines
                .push(Line::Provisional { name: turn.name, text: turn.text.bytes(), cut: turn.cut })
                .expect("live projection fits");
        }
        let dropped = lines.len().saturating_sub(self.limits.lines);
        let mut kept = List::with_capacity(self.limits.lines.saturating_add(1));
        let left = self.left.saturating_add(dropped);
        if left > 0 {
            kept.push(Line::Left { lines: left }).expect("loss marker fits");
        }
        for (at, line) in lines.into_boxed().into_iter().enumerate() {
            if u32::try_from(at).expect("bounded projection index fits") >= dropped {
                kept.push(line).expect("retained projection fits");
            }
        }
        kept
    }
}
fn number(value: &OwnerOutcome, task: u64, place: u32) -> Option<u64> {
    match value {
        OwnerOutcome::Said { task: written, message } => {
            if *written == task {
                Some(*message)
            } else {
                None
            }
        }
        OwnerOutcome::Compound { operations, first_message, .. } => {
            if place < *operations {
                first_message.checked_add(u64::from(place))
            } else {
                None
            }
        }
        OwnerOutcome::Created { .. }
        | OwnerOutcome::MarkedSeen { .. }
        | OwnerOutcome::NoteEdited { .. }
        | OwnerOutcome::ServiceMade { .. }
        | OwnerOutcome::Adopted { .. }
        | OwnerOutcome::Stopped { .. }
        | OwnerOutcome::Cancelled { .. }
        | OwnerOutcome::Placed { .. }
        | OwnerOutcome::Lifted { .. }
        | OwnerOutcome::Released { .. }
        | OwnerOutcome::PersonTaken { .. }
        | OwnerOutcome::PersonHandedBack { .. }
        | OwnerOutcome::PersonAnswered { .. }
        | OwnerOutcome::Moved { .. }
        | OwnerOutcome::ProposalDecided { .. }
        | OwnerOutcome::QuestionAnswered { .. }
        | OwnerOutcome::Amended { .. }
        | OwnerOutcome::Proposed { .. }
        | OwnerOutcome::ReleaseProposed { .. }
        | OwnerOutcome::Withdrawn { .. }
        | OwnerOutcome::RolesSet { .. }
        | OwnerOutcome::PolicyChanged { .. }
        | OwnerOutcome::AccountProgramDeclared { .. }
        | OwnerOutcome::AccountKeyKept { .. }
        | OwnerOutcome::AccountFinishing { .. }
        | OwnerOutcome::AccountMovedIn { .. }
        | OwnerOutcome::AccountChanged
        | OwnerOutcome::AccountRemoved { .. }
        | OwnerOutcome::AccountSignInStarted { .. }
        | OwnerOutcome::AccountSignedOut { .. }
        | OwnerOutcome::Renamed { .. }
        | OwnerOutcome::ProjectMade { .. }
        | OwnerOutcome::PoolSet { .. }
        | OwnerOutcome::EscalationDecided { .. } => None,
    }
}
