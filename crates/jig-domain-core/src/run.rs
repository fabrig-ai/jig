//! Live run and restart correlation held by the core (domain/engine.md, 3 and 7).

use crate::{CallKey, Core, TurnRecord};
use alloc::boxed::Box;
use jig_domain_authority as authority;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use skein_lib::{List, Queue, Token, Wall};

use crate::transcript_text::TranscriptText;
use crate::translate;

/// The core's decision before a due agent run is prepared or claimed.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum RunAdmission {
    /// Current policy, funds, account and writes allow preparation.
    Allow(Box<crate::RunAllowance>),
    /// A current account, application credential or host wait.
    Wait(crate::RunWait),
    /// The task child must hold the run for the named reason.
    Hold(tasks::Hold),
}

/// Bounded recent opaque conversation while a due task's store pages are read.
/// Each retained turn counts its four-byte length beside its opaque body
/// (domain/engine.md, 7.2).
#[derive(Debug)]
pub struct Transcript {
    pub window: Option<(u64, u32)>,
    /// An upgrade closed the loaded window or crossed its claim.
    pub closed: bool,
    pub previous_attempt: u64,
    pub last: Option<(u64, u32)>,
    pub from: u64,
    pub bytes: u64,
    pub kept: u64,
    pub turns: Queue<Box<[u8]>>,
    pub texts: Queue<TranscriptText>,
    pub text_bytes: u64,
    pub text_kept: u64,
}

impl Core {
    pub(crate) fn run_credentials(&self, task: u64) -> &[u32] {
        if let Some(credentials) = self.workspace_credentials.get(&task) {
            return credentials;
        }
        match self.proofs.get(&task) {
            Some(proof) => &proof.credentials,
            None => &[],
        }
    }
    pub(crate) fn credentials_ready(&self, task: u64) -> bool {
        for number in self.run_credentials(task) {
            if !self.accounts.credential_usable(*number) {
                return false;
            }
        }
        true
    }
    pub(crate) fn run_credentials_check(&self, task: u64) -> Box<[bool]> {
        let mut usable = List::with_capacity(self.accounts.credential_limit());
        for number in self.run_credentials(task) {
            usable.push(self.accounts.credential_usable(*number)).expect("credential bound");
        }
        usable.into_boxed()
    }
    /// Decide a run against current policy, funds, account usability and
    /// connector-translated workspace writes.
    #[must_use]
    #[expect(
        clippy::too_many_lines,
        reason = "one run answer preserves deadline, budget, writer and readiness precedence"
    )]
    pub fn run_admission(&self, task: &tasks::RunContext, wall: Wall, writes: Box<[authority::Write]>) -> RunAdmission {
        let Some(task) = self.tasks.run_context(task.task) else { return RunAdmission::Wait(crate::RunWait::Host) };
        let choice = self.run_choice(task.task);
        let mut accounts = List::with_capacity(jig_domain_tasks::MODELS);
        if let Ok(choice) = &choice {
            for binding in &choice.accounts {
                accounts
                    .push(authority::AccountCheck { binding: *binding, usable: true, host_eligible: true })
                    .expect("provider bound");
            }
        }
        let mut findings =
            Queue::with_capacity(authority::max_out(self.authority.limits()).expect("authority check bound"));
        let mut current = List::with_capacity(self.authority.limits().writes);
        for mut write in writes {
            write.effect = crate::resources::effect(self, task.project, write.effect);
            if current.push(write).is_err() {
                return RunAdmission::Hold(tasks::Hold::Effects);
            }
        }
        let writes = current.into_boxed();
        let numbers = translate::authority_numbers(task.numbers);
        let checked = authority::check_run(
            &self.authority,
            &authority::RunAsk {
                project: task.project,
                authority: translate::authority_value(&task.authority),
                numbers,
                budget: authority::left(numbers).min(self.authority.rules().maximum_run_spend),
                reserve: self.run_reserve(task.task),
                wall,
                main_provider: match &choice {
                    Ok(choice) => choice.accounts.first().expect("choice includes main provider").provider,
                    Err(_) => 0,
                },
                accounts: accounts.into_boxed(),
                credentials: self.run_credentials_check(task.task),
                writes,
            },
            &mut findings,
        );
        match checked {
            authority::RunAnswer::Allow { spend, accounts } => {
                let mut choice = choice.expect("authority requires a usable main binding");
                choice.accounts = accounts;
                RunAdmission::Allow(Box::new(crate::RunAllowance { choice, spend }))
            }
            authority::RunAnswer::Wait | authority::RunAnswer::Propose | authority::RunAnswer::Refuse => {
                let mut account = false;
                let mut hold = None;
                for _ in 0..findings.len() {
                    match findings.pop().expect("authority finding count") {
                        authority::Finding::Account => account = true,
                        authority::Finding::Deadline => hold = Some(tasks::Hold::Deadline),
                        authority::Finding::RunBudget
                        | authority::Finding::RunCap
                        | authority::Finding::Arithmetic
                        | authority::Finding::Spend { .. }
                        | authority::Finding::Price { .. } => {
                            if hold != Some(tasks::Hold::Deadline) {
                                hold = Some(tasks::Hold::Budget);
                            }
                        }
                        authority::Finding::ResourceUnavailable => {
                            if hold.is_none()
                                || hold == Some(tasks::Hold::Effects)
                                || hold == Some(tasks::Hold::ResourceAccess)
                            {
                                hold = Some(tasks::Hold::ResourceUnavailable);
                            }
                        }
                        authority::Finding::ResourceAccess => {
                            if hold.is_none() || hold == Some(tasks::Hold::Effects) {
                                hold = Some(tasks::Hold::ResourceAccess);
                            }
                        }
                        authority::Finding::Oversized
                        | authority::Finding::UnknownProject
                        | authority::Finding::UnknownRole
                        | authority::Finding::Authority { .. }
                        | authority::Finding::Executor { .. }
                        | authority::Finding::Tasks { .. }
                        | authority::Finding::Writer
                        | authority::Finding::Tool
                        | authority::Finding::Grant { .. }
                        | authority::Finding::Reference
                        | authority::Finding::Scope { .. }
                        | authority::Finding::Required { .. }
                        | authority::Finding::Failed { .. }
                        | authority::Finding::Unguarded { .. }
                        | authority::Finding::Unpermitted
                        | authority::Finding::Undecidable
                        | authority::Finding::PeriodSpend => {
                            if hold.is_none() {
                                hold = Some(tasks::Hold::Effects);
                            }
                        }
                    }
                }
                match hold {
                    Some(why) => RunAdmission::Hold(why),
                    None if account => RunAdmission::Wait(match choice {
                        Err(wait) => wait,
                        Ok(_) => crate::RunWait::Credential,
                    }),
                    None => RunAdmission::Hold(tasks::Hold::Effects),
                }
            }
        }
    }

    /// Put back the durable evidence when a pending claim cannot commit.
    pub(crate) fn restore_replaced_proof(&mut self, task: u64) {
        drop(self.proofs.remove(&task));
        if let Some(proof) = self.replaced.remove(&task) {
            self.proofs.insert(task, proof).expect("replacement releases proof room");
        }
    }

    /// Begin a bounded transcript load for a due task's previous attempt.
    #[expect(clippy::manual_map, reason = "the strict subset translates windows with an explicit match")]
    pub fn begin_transcript(&mut self, task: u64, previous_attempt: u64) {
        let window = match self.proofs.get(&task) {
            Some(proof) => match proof.window {
                Some(window) => Some((window.attempt, window.turn)),
                None => None,
            },
            None => None,
        };
        let closed = match self.proofs.get(&task) {
            Some(proof) => {
                proof.crossed
                    || match proof.window {
                        Some(window) => window.closed,
                        None => false,
                    }
            }
            None => false,
        };
        let from = match self.proofs.get(&task) {
            Some(proof) => proof.conversation,
            None => 1,
        };
        assert!(
            self.transcripts
                .insert(
                    task,
                    Transcript {
                        window,
                        closed,
                        previous_attempt,
                        last: None,
                        from,
                        bytes: 0,
                        kept: 0,
                        texts: Queue::with_capacity(self.settings.tail_bytes),
                        text_bytes: 0,
                        text_kept: 0,
                        turns: Queue::with_capacity(
                            self.settings.resume_bytes.checked_div(4).expect("nonzero length size")
                        ),
                    },
                )
                .is_ok(),
            "one transcript preparation per live task"
        );
    }

    /// Admit one restored turn in store order, retaining only the bounded
    /// tail while still counting the full current window for resume policy.
    /// Text is UTF-8 supplied by the host protocol; `None` keeps an unreadable marker.
    #[must_use]
    pub fn append_transcript(&mut self, task: u64, turn: TurnRecord, text: Option<Box<[u8]>>) -> bool {
        let transcript = self.transcripts.get_mut(&task).expect("load belongs to prepared task");
        if turn.task != task || turn.attempt > transcript.previous_attempt || turn.attempt == 0 || turn.turn == 0 {
            return false;
        }
        if let Some(previous) = transcript.last
            && (turn.attempt, turn.turn) <= previous
        {
            return false;
        }
        let Some(start) = transcript.window else { return false };
        if (turn.attempt, turn.turn) < start
            || (transcript.last.is_none() && ((turn.attempt, turn.turn) != start || !turn.opens_window))
        {
            return false;
        }
        let length = u64::try_from(turn.transcript.len()).expect("stored turn size fits u64");
        let Some(framed) = length.checked_add(4) else { return false };
        let Some(total) = transcript.bytes.checked_add(framed) else { return false };
        transcript.bytes = total;
        transcript.last = Some((turn.attempt, turn.turn));
        transcript.append_text(text, self.settings.tail_bytes);
        if transcript.closed {
            return true;
        }
        let bound = u64::from(self.settings.resume_bytes);
        if bound < 4 {
            return true;
        }
        let body_bound = bound.checked_sub(4).expect("room for one length");
        let body = if length > body_bound {
            let start = turn
                .transcript
                .len()
                .checked_sub(usize::try_from(body_bound).expect("u32 bound fits usize"))
                .expect("oversize turn has tail");
            Box::<[u8]>::from(turn.transcript.get(start..).expect("tail starts inside stored turn"))
        } else {
            turn.transcript
        };
        let body_len =
            u64::try_from(body.len()).expect("bounded turn fits u64").checked_add(4).expect("bounded framed turn");
        for _ in 0..transcript.turns.len() {
            if transcript.kept.checked_add(body_len).expect("two bounded tails") <= bound {
                break;
            }
            let old = transcript.turns.pop().expect("overfull transcript has an older turn");
            transcript.kept = transcript
                .kept
                .checked_sub(
                    u64::try_from(old.len()).expect("bounded old turn").checked_add(4).expect("bounded old frame"),
                )
                .expect("old turn was counted");
        }
        transcript.turns.push(body);
        transcript.kept = transcript.kept.checked_add(body_len).expect("bounded transcript tail");
        true
    }

    /// A completed load includes the window the core asked the store to read.
    #[must_use]
    pub fn transcript_readable(&self, task: u64) -> bool {
        let transcript = self.transcripts.get(&task).expect("prepared transcript state");
        transcript.window.is_none() || transcript.last.is_some()
    }

    /// Whether the current window exceeded the deployment's resume bound.
    #[must_use]
    pub fn transcript_oversized(&self, task: u64) -> bool {
        self.transcripts.get(&task).expect("prepared transcript state").bytes > u64::from(self.settings.resume_bytes)
    }

    /// Select whole prior turns only while their cumulative bytes fit the
    /// deployment's resume bound. A longer window starts fresh.
    #[must_use]
    pub fn resumed_turns(&self, transcript: Transcript) -> Box<[Box<[u8]>]> {
        let mut turns = List::with_capacity(transcript.turns.len());
        if !transcript.closed && transcript.bytes <= u64::from(self.settings.resume_bytes) {
            let mut kept = transcript.turns;
            for _ in 0..kept.len() {
                turns.push(kept.pop().expect("counted transcript turn")).expect("bounded transcript turns");
            }
        }
        turns.into_boxed()
    }
}

#[derive(Debug)]
pub struct RestoringProof {
    pub attempt: u64,
    pub turn: u32,
    pub run_spent: u64,
    pub last_answer: Option<u64>,
}

/// One configured model and its deployment-unit prices (domain/agent.md, 4.6).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Model {
    pub dialect: u32,
    pub endpoint: u32,
    /// The endpoint's backend, when it takes program sources (configuration.md, 3).
    pub program: Option<Box<[u8]>>,
    pub name: Box<[u8]>,
    /// The input tokens a conversation's window may hold on this model.
    pub window: u32,
    /// The most output tokens one completion may have, thinking included.
    pub output: u32,
    pub input_price: u64,
    pub cached_price: u64,
    /// Cache writes, when priced apart from fresh input.
    pub cache_write_price: Option<u64>,
    pub output_price: u64,
    pub price_unit: u32,
    /// The efforts the model declares, by name in its order; a task's LLM choice names one by
    /// index, and none for a model that declares none (domain/tasks.md, 3).
    pub efforts: Box<[Box<[u8]>]>,
    /// The effort a run uses on this model, one of `efforts` by name, or `None` for the LLM
    /// provider's default: a run's charter carries its task's choice's (domain/engine.md, 7.1).
    /// Configured models and charters carry none: configuration names a charter's models, not
    /// an effort for each (configuration.md, 3).
    pub effort: Option<Box<[u8]>>,
}

/// The most efforts a configured model declares (configuration.md, 3).
pub const EFFORTS: u32 = 16;
/// The most bytes in an effort's name: the bound smith's charter holds an effort to
/// (smith's protocol/charter.md, sections 2 and 6).
pub const EFFORT_BYTES: u32 = 32;
/// Byte bounds on a configured model's name and description (configuration.md, 6).
pub use jig_domain_people::{MODEL_DESCRIPTION_BYTES, MODEL_NAME_BYTES};

/// Root-owned charter policy, independent of Smith's run vocabulary.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[expect(clippy::struct_excessive_bools, reason = "independent charter grants and run behavior")]
pub struct RunPolicy {
    pub instructions: Box<[u8]>,
    pub waiting: skein_lib::Duration,
    pub resume: bool,
    pub turns: u32,
    pub time: skein_lib::Duration,
    /// The turns and time of each run kept for finishing once the rest of
    /// its budget is spent.
    pub reserve_turns: u32,
    pub reserve_time: skein_lib::Duration,
    pub model: Model,
    pub alternatives: Box<[Model]>,
    pub inspect: bool,
    pub modify: bool,
    pub shell: bool,
    pub agents: bool,
    pub call_timeout: skein_lib::Duration,
}

/// The root's complete task-specific charter, kept behind one bounded
/// assignment cell so delivery queues carry a small fixed-size value. Its settings
/// revision is fixed when preparation begins (domain/engine.md, 17.3).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RunCharter {
    /// The settings revision recorded as this run began preparation.
    pub revision: u32,
    pub ended_by: tasks::EndedBy,
    /// Selected grant for main then each retained alternative; none sources have no grant.
    pub sources: Box<[Option<u32>]>,
    pub accounts: Box<[authority::AccountBinding]>,
    pub placement: jig_domain_fleet::PlacementConstraint,
    pub policy: RunPolicy,
    pub contract: tasks::Contract,
    pub authority: tasks::Authority,
    pub budget: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RoutedCall {
    Escalation { key: CallKey, task: u64, revision: u64, passed_up: bool },
    Propose { key: CallKey, proposal: u64 },
    Decide { key: CallKey, proposal: u64 },
    Withdraw { key: CallKey, proposal: u64, moved: Option<(u64, people::Moved)> },
    Accepting { key: CallKey, proposer: u64, proposal: u64, message: u64 },
    Message(CallKey),
    Introduce(CallKey),
    Subscribe { key: CallKey, subscription: u64 },
    Unsubscribe(CallKey),
    Control { key: CallKey, target: u64, revised: bool },
}

#[derive(Debug)]
pub struct HistoricalResult {
    pub task: u64,
    pub kind: tasks::ResultKind,
    pub words: Box<[u8]>,
}

#[derive(Debug)]
pub struct PendingRelay {
    pub previous: Option<u64>,
    pub message: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PersonProposalRoute {
    Deciding { request: Token, proposer: u64, proposal: u64, by: u64, moved: Option<people::Moved> },
    Accepting { request: Token, person: u64, proposer: u64, proposal: u64, message: u64 },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PersonTaskRoute {
    Compound { project: u32, operations: u32, first_message: u64 },
    PoolSet { project: u32, person: u64 },
    Take(u64),
    HandBack(u64),
    Answer(u64),
    Cancel(u64),
    Release(u64),
    Amended { task: u64, released: bool },
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum GoalRoute {
    Offering { proposal: u64, to: u32, stubs: Box<[tasks::Stub]> },
    Withdrawing { proposer: u64, proposal: u64, moved: Option<people::Moved> },
    Accepting { proposer: u64, proposal: u64, by: u64 },
    Deciding { proposer: u64, proposal: u64, by: u64 },
}

/// A completion's maximum price, rounded up once (domain/authority.md, 7).
#[must_use]
pub fn most_costly(model: &Model) -> Option<u64> {
    let rate = model.input_price.max(model.cached_price).max(model.cache_write_price.unwrap_or(model.input_price));
    let input = u128::from(model.window).checked_mul(u128::from(rate))?;
    let output = u128::from(model.output).checked_mul(u128::from(model.output_price))?;
    let sum = input.checked_add(output)?;
    let unit = u128::from(model.price_unit);
    let rounded = sum.checked_div(unit)?.checked_add(u128::from(sum.checked_rem(unit)? != 0))?;
    u64::try_from(rounded).ok()
}

/// Reserve turns and one completion more (domain/authority.md, 7).
#[must_use]
pub fn reserve_floor(policy: &RunPolicy) -> Option<u64> {
    most_costly(&policy.model)?.checked_mul(u64::from(policy.reserve_turns).checked_add(1)?)
}

impl Core {
    /// The spend a run of `task` reserves: its reserve turns and one completion more of its LLM
    /// choice's main model (domain/authority.md, 7 and 8.3).
    pub(crate) fn run_reserve(&self, task: u64) -> u64 {
        // No spend holds a reserve beyond the numbers' range, so no run of it starts.
        reserve_floor(&self.chosen_policy(task)).unwrap_or(u64::MAX)
    }
}
