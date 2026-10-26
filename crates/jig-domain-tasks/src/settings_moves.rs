//! Moving live agent tasks to the newest settings (domain/engine.md, 17.4).
use crate::domain::{Domain, task_mut};
use crate::{CharterModels, Executor, Limits, Request, Stored, TaskRecord};
use alloc::boxed::Box;
use skein_lib::{Env, List, Map, Queue};

fn may(task: &TaskRecord, newest: u32, charters: &[CharterModels]) -> bool {
    let Executor::Agent { charter } = task.executor else { return false };
    let mut models = None;
    let mut latest = 0;
    for version in charters {
        if version.number == charter && version.revision <= newest && version.revision >= latest {
            latest = version.revision;
            models = version.models.as_deref();
        }
    }
    let Some(models) = models else { return false };
    let Some(choice) = &task.llm else { return true };
    if !models.contains(&choice.main.model) {
        return false;
    }
    for subagent in &choice.subagents {
        if !models.contains(&subagent.model) {
            return false;
        }
    }
    true
}

fn selected(task: &TaskRecord, from: Option<u32>, newest: u32) -> bool {
    task.settings.task < newest
        && match from {
            Some(revision) => task.settings.task == revision,
            None => true,
        }
}

impl Domain {
    /// Whether the newest offers this live agent task's charter and allows its LLM choice.
    #[must_use]
    pub fn may_move(&self, task: u64) -> bool {
        match self.task(task) {
            Some(task) => may(task, self.newest(), &self.charters),
            None => false,
        }
    }

    /// Revisions besides `newest` still held after the selected moves. The caller
    /// supplies room for `revisions + 1` names and the prospective charter view.
    pub fn held_after_moves(
        &self,
        from: Option<u32>,
        newest: u32,
        charters: &[CharterModels],
        into: &mut Map<u32, ()>,
    ) {
        while let Some((&revision, ())) = into.first() {
            into.remove(&revision);
        }
        for (&number, _) in &self.names {
            let task = self.task(number).expect("live task name");
            let Executor::Agent { .. } = task.executor else { continue };
            if task.settings.task != newest && !(selected(task, from, newest) && may(task, newest, charters)) {
                into.insert(task.settings.task, ()).expect("bounded held revisions");
            }
            if let Some(run) = task.settings.run
                && run != newest
            {
                into.insert(run, ()).expect("bounded held revisions");
            }
        }
    }
}

pub(crate) fn move_tasks(domain: &mut Domain, env: &Env<Limits>, from: Option<u32>, out: &mut Queue<Request>) {
    let mut moving = List::with_capacity(env.limits.tasks);
    let mut stayed = 0_u32;
    for (&number, _) in &domain.names {
        let task = domain.task(number).expect("live task name");
        let Executor::Agent { .. } = task.executor else { continue };
        if !selected(task, from, domain.newest()) {
            continue;
        }
        if domain.may_move(number) {
            moving.push(number).expect("bounded live tasks");
        } else {
            stayed = stayed.checked_add(1).expect("bounded live tasks");
        }
    }
    let newest = domain.newest();
    for &number in &moving {
        task_mut(domain, number).expect("moving task").record.settings.task = newest;
    }
    crate::settings::recount(domain);
    for &number in &moving {
        let task = task_mut(domain, number).expect("moving task");
        crate::changed::observe(task, env.wall);
        out.push(Request::Save { record: Stored::Live(Box::new(task.record.clone())) });
    }
    out.push(Request::MovedToNewest { moved: moving.len(), stayed });
}
