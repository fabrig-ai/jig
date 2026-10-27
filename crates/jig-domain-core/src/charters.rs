//! Configured charters and readiness (domain/engine.md, 7.1 and 8).

use alloc::boxed::Box;
use jig_domain_fleet as fleet;
use jig_domain_tasks as tasks;
use skein_lib::{List, Queue};

use crate::{Core, Limits, RunPolicy, Settings};

/// A connector resource, path and optional heading a charter names (engine.md, 17.9).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Document {
    pub connector: u16,
    pub name: Box<[Box<[u8]>]>,
    /// At most 256 bytes.
    pub path: Box<[u8]>,
    /// At most 80 bytes.
    pub heading: Option<Box<[u8]>>,
}

/// One numbered charter offered by the deployment (engine.md, 17.9).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Charter {
    pub number: u32,
    pub policy: RunPolicy,
    pub hosts: fleet::Kinds,
    pub spend: u64,
    pub documents: Box<[Document]>,
}

/// Metadata owned by one charter at its shape bounds (engine.md, 17.9).
#[must_use]
pub fn documents_worst_case(limits: &Limits) -> Option<u64> {
    let name = u64::from(limits.authority.segments).checked_mul(
        u64::try_from(size_of::<Box<[u8]>>()).ok()?.checked_add(u64::from(limits.authority.segment_bytes))?,
    )?;
    u64::from(limits.tasks.charter_documents)
        .checked_mul(u64::try_from(size_of::<Document>()).ok()?.checked_add(name)?.checked_add(256 + 80)?)
}

const _: () = assert!(fleet::CATALOGUE_MODELS == tasks::MODELS, "fleet evidence prices the model catalogue");
const _: () = assert!(fleet::CATALOGUE_NAME_BYTES == crate::MODEL_NAME_BYTES, "fleet evidence prices model names");

pub(crate) fn assert_catalogue(settings: &Settings) {
    assert!(settings.models.len() <= usize::try_from(tasks::MODELS).expect("model count"), "model count");
    for (at, model) in settings.models.iter().enumerate() {
        for earlier in settings.models.iter().take(at) {
            assert!(earlier.name != model.name, "unique configured model names");
        }
        assert!(model.name.len() <= usize::try_from(crate::MODEL_NAME_BYTES).expect("name bound"), "model name bound");
        if let Some(program) = &model.program {
            assert!(
                !program.is_empty()
                    && program.len() <= usize::try_from(jig_domain_accounts::NAME).expect("program bound"),
                "program name bound"
            );
        }
        assert!(model.efforts.len() <= usize::try_from(crate::EFFORTS).expect("effort count"), "effort count");
        for (at, effort) in model.efforts.iter().enumerate() {
            assert!(
                !effort.is_empty() && effort.len() <= usize::try_from(crate::EFFORT_BYTES).expect("effort bound"),
                "effort name bound"
            );
            for earlier in model.efforts.iter().take(at) {
                assert!(earlier != effort, "unique effort names");
            }
        }
        assert!(declares(model), "a model's effort among those it declares");
    }
}

/// Each charter's models as tasks checks an LLM choice against them: its main model, then its
/// alternatives, each once, by its index in configuration's order (tasks.md, 3).
pub(crate) fn models(
    current: &crate::settings::Current,
    settings: &Settings,
    limits: &Limits,
) -> Box<[tasks::CharterModels]> {
    let mut charters = List::with_capacity(
        limits
            .tasks
            .charters
            .checked_mul(2)
            .expect("newest charter states")
            .checked_add(limits.settings.versions)
            .expect("charter version bound"),
    );
    append_models(current, settings, &mut charters);
    charters.into_boxed()
}

fn append_models(current: &crate::settings::Current, settings: &Settings, charters: &mut List<tasks::CharterModels>) {
    for setting in &current.offered() {
        let number = match setting {
            crate::settings::Setting::Charter(number) => *number,
            crate::settings::Setting::Template(_)
            | crate::settings::Setting::Policy(_)
            | crate::settings::Setting::Rules => continue,
        };
        let charter = current.charter(number).expect("offered charter");
        let (_, origin) = current.lookup(*setting);
        let (_, revision) = origin.expect("offered charter version");
        let value = one_models(charter, revision, settings);
        charters.push(value).expect("configured charter count");
    }
    for (setting, change) in &current.changes {
        if let crate::settings::Setting::Charter(number) = setting
            && change.value.is_none()
        {
            charters
                .push(tasks::CharterModels { number: *number, revision: change.revision, models: None })
                .expect("removed charter version bound");
        }
    }
}

pub(crate) fn one_models(charter: &Charter, revision: u32, settings: &Settings) -> tasks::CharterModels {
    let count = charter.policy.alternatives.len().checked_add(1).expect("charter model count");
    let mut models = List::with_capacity(u32::try_from(count).expect("charter model count"));
    if let Some(main) = configured(settings, &charter.policy.model) {
        models.push(main).expect("charter model count");
    }
    for model in &charter.policy.alternatives {
        let Some(index) = configured(settings, model) else { continue };
        if !models.as_slice().contains(&index) {
            models.push(index).expect("charter model count");
        }
    }
    tasks::CharterModels { number: charter.number, revision, models: Some(models.into_boxed()) }
}

/// How many efforts each configured model declares, in configuration's order (tasks.md, 3).
pub(crate) fn efforts(settings: &Settings) -> Box<[u8]> {
    let mut efforts = List::with_capacity(u32::try_from(settings.models.len()).expect("model count"));
    for model in &settings.models {
        efforts.push(u8::try_from(model.efforts.len()).expect("efforts named by a byte")).expect("model count");
    }
    efforts.into_boxed()
}

/// Whether `model`'s effort, if any, is one it declares (domain/tasks.md, 3).
fn declares(model: &crate::Model) -> bool {
    match &model.effort {
        Some(effort) => model.efforts.contains(effort),
        None => true,
    }
}

/// A model's index in configuration's order, matched by name.
pub(crate) fn configured(settings: &Settings, model: &crate::Model) -> Option<u8> {
    for (index, configured) in settings.models.iter().enumerate() {
        if configured.name == model.name {
            return u8::try_from(index).ok();
        }
    }
    None
}

impl Core {
    pub(crate) fn claim_done(&mut self, task: u64, attempt: u64, out: &mut Queue<crate::Request>) {
        drop(self.replaced.remove(&task));
        let proof = self.proofs.get(&task).expect("claim proof pre-reserved");
        out.push(crate::Request::Write(crate::Write::Save(crate::Record::Core(crate::CoreRecord::RunProof(
            proof.clone(),
        )))));
        let kinds = match proof.host {
            fleet::HostKind::Engine => fleet::Kinds::Engine,
            fleet::HostKind::Worker => fleet::Kinds::Workers,
        };
        let version = self.charter_version(task);
        for &connector in self.connectors.as_ref() {
            out.push(crate::Request::Ask { connector, ask: crate::Ask::ClaimDone { task, attempt, kinds, version } });
        }
    }

    /// The value read by this live agent's run, or its task before preparation.
    pub(crate) fn charter_version(&self, task: u64) -> fleet::Version {
        let row = self.tasks.task(task).expect("live agent task");
        let charter = match row.executor {
            tasks::Executor::Agent { charter } => charter,
            tasks::Executor::Procedure { .. } | tasks::Executor::Person(_) => unreachable!("agent version"),
        };
        let revision = row.settings.run.unwrap_or(row.settings.task);
        let key = self.held.selected(&self.current, charter, revision).expect("run charter version");
        fleet::Version { charter: key.charter, revision: key.revision }
    }

    /// The newest charter, for work about to be made (engine.md, 17.3).
    #[must_use]
    pub fn charter(&self, number: u32) -> Option<&Charter> {
        self.current.charter(number)
    }

    /// An admitted live agent task's run revision, or its own when no run lives.
    #[must_use]
    pub fn charter_of(&self, task: u64) -> &Charter {
        let row = self.tasks.task(task).expect("live agent task");
        let revision = row.settings.run.unwrap_or(row.settings.task);
        let charter = match row.executor {
            tasks::Executor::Agent { charter } => self.charter_at(charter, revision),
            tasks::Executor::Procedure { .. } | tasks::Executor::Person(_) => None,
        };
        charter.expect("agent task has configured charter")
    }

    /// Whether a context's task is still due; one that left Due activates
    /// afresh if it is due again.
    fn still_due(&self, context: &tasks::RunContext) -> bool {
        let Some(current) = self.tasks.task(context.task) else { return false };
        current.phase == tasks::Phase::Active(tasks::Active::Due)
    }

    fn context_ready(&self, context: &tasks::RunContext) -> bool {
        if !self.still_due(context) {
            return false;
        }
        match self.tasks.task(context.task).expect("a due task is live").executor {
            tasks::Executor::Agent { .. } => match self.run_admission(context, self.run_wall, Box::new([])) {
                crate::RunAdmission::Allow(_) | crate::RunAdmission::Hold(_) => true,
                crate::RunAdmission::Wait(_) => false,
            },
            tasks::Executor::Procedure { .. } => true,
            tasks::Executor::Person(_) => false,
        }
    }

    /// Take the first due task whose executor can start, keeping the others in
    /// order and dropping those whose task is no longer due.
    pub fn take_due(&mut self) -> Option<Box<tasks::RunContext>> {
        let mut taken = None;
        for _ in 0..self.due.len() {
            let context = self.due.pop().expect("due count");
            if !self.still_due(&context) {
                continue;
            }
            if taken.is_none() && self.context_ready(&context) {
                taken = self.tasks.run_context(context.task);
            } else {
                self.due.push(context);
            }
        }
        taken
    }

    /// Whether a due procedure or agent with a usable account can start.
    #[must_use]
    pub fn due_ready(&self) -> bool {
        for context in &self.due {
            if self.context_ready(context) {
                return true;
            }
        }
        false
    }
}

/// The newest view plus every retained value and erased-removal barrier.
pub(crate) fn held_models(core: &Core, limits: &Limits) -> Box<[tasks::CharterModels]> {
    let mut values = List::with_capacity(
        limits
            .tasks
            .charters
            .checked_mul(2)
            .expect("newest charter states")
            .checked_add(limits.settings.versions)
            .expect("version models"),
    );
    append_models(&core.current, &core.settings, &mut values);
    for (&key, value) in &core.held.versions {
        let models = match &value.charter {
            Some(charter) => one_models(charter, key.revision, &core.settings),
            None => tasks::CharterModels { number: key.charter, revision: key.revision, models: None },
        };
        values.push(models).expect("held model table");
    }
    values.into_boxed()
}
