//! The checked copy serves restores before its explicit switch (store.md 10.2, 5–8).
use super::plan::{self, Levels};
use crate::codec::component::Count;
use crate::directory::Opening;
use crate::generations::{self, Manifesting, Progress, Switching};
use crate::{Arrived, Below, Event, Limits, OpenRefusal, Phase, Store, directory, open};
use alloc::boxed::Box;
use skein_lib::{Env, List, Queue, Token};

#[derive(Debug)]
pub(crate) enum Stage {
    Opening,
    Pending,
    Waiting { owner: Token, through: u64 },
    Manifest { owner: Token, operation: Manifesting },
    Switching { owner: Token, operation: Switching },
}

/// Active's number and expected digest remain in Directory until Switched.
/// Retain the check's counts and both levels while the root changes the copy.
#[derive(Debug)]
pub(crate) struct Pending {
    pub before: Levels,
    pub after: Levels,
    pub counts: List<Count>,
    pub stage: Stage,
}

pub(crate) fn pending(store: &Store) -> bool {
    directory::pending(store).is_some()
}

pub(crate) fn open(
    store: &mut Store,
    before: Levels,
    counts: List<Count>,
    above: &mut Queue<Event>,
    below: &mut Below,
) {
    store.directory.opening = Opening::Pending(Box::new(Pending {
        before,
        after: plan::binary(&store.patch_sets),
        counts,
        stage: Stage::Opening,
    }));
    // Decode installation and header, construct serving machines and next, then
    // open the shared transcripts root and cut its tails, all over the open copy.
    store.phase = Phase::Closed;
    open::open(store, store.directory.owner, above, below);
}

pub(crate) fn opened(store: &mut Store) {
    if let Some(pending) = directory::pending_mut(store) {
        pending.stage = Stage::Pending;
    }
}

pub(crate) fn ask(store: &mut Store, env: &Env<Limits>, owner: Token, above: &mut Queue<Event>, below: &mut Below) {
    let through = match store.phase {
        Phase::Ready { next, .. } => next.checked_sub(1).expect("ready has a next commit"),
        Phase::Closed | Phase::Stopped => return open::refuse(store, owner, OpenRefusal::Failed, above),
    };
    let Some(pending) = directory::pending_mut(store) else {
        return open::refuse(store, owner, OpenRefusal::Failed, above);
    };
    match pending.stage {
        Stage::Pending => pending.stage = Stage::Waiting { owner, through },
        Stage::Opening | Stage::Waiting { .. } | Stage::Manifest { .. } | Stage::Switching { .. } => {
            return open::refuse(store, owner, OpenRefusal::Failed, above);
        }
    }
    resume(store, env, below);
}

pub(crate) fn resume(store: &mut Store, env: &Env<Limits>, below: &mut Below) {
    if store.phase == Phase::Stopped {
        return;
    }
    let Some(pending) = directory::pending(store) else { return };
    let owner = match pending.stage {
        Stage::Waiting { owner, through } => {
            if store.stores.store.last() < through || !crate::secrets::settled(store) {
                return;
            }
            owner
        }
        Stage::Opening | Stage::Pending | Stage::Manifest { .. } | Stage::Switching { .. } => return,
    };
    let active = store.directory.active.expect("pending open retains old active");
    let number = active.number.checked_add(1).expect("patch begin checked next generation");
    let bytes = generations::manifest(
        number,
        Some(active.number),
        pending.before,
        pending.after,
        pending.counts.as_slice(),
        store.stores.store.last(),
        Some(store.stores.secrets.last()),
        env.wall,
    );
    let root = store.directory.root.expect("pending open holds the deployment lock");
    let operation = generations::store_manifest(root, number, bytes, below);
    directory::pending_mut(store).expect("pending retained").stage = Stage::Manifest { owner, operation };
}

pub(crate) fn up(store: &mut Store, arrived: Arrived, above: &mut Queue<Event>, below: &mut Below) {
    let stage =
        core::mem::replace(&mut directory::pending_mut(store).expect("pending switch routing").stage, Stage::Opening);
    match stage {
        Stage::Manifest { owner, mut operation } => match generations::manifested(&mut operation, arrived) {
            Progress::Pending => {
                directory::pending_mut(store).expect("pending retained").stage = Stage::Manifest { owner, operation };
            }
            Progress::Done => {
                let root = store.directory.root.expect("pending open holds the lock");
                let active = store.directory.active.expect("old active retained until switch");
                let operation = generations::switch(root, active, below);
                match operation.progress() {
                    Progress::Pending => {
                        directory::pending_mut(store).expect("pending retained").stage =
                            Stage::Switching { owner, operation };
                    }
                    Progress::Done | Progress::Failed => open::refuse(store, owner, OpenRefusal::Failed, above),
                }
            }
            Progress::Failed => open::refuse(store, owner, OpenRefusal::Failed, above),
        },
        Stage::Switching { owner, mut operation } => match generations::switched(&mut operation, arrived) {
            Progress::Pending => {
                directory::pending_mut(store).expect("pending retained").stage = Stage::Switching { owner, operation };
            }
            Progress::Done => {
                store.directory.active = operation.active();
                store.directory.opening = Opening::Idle;
                above.push(Event::Switched { owner });
            }
            Progress::Failed => open::refuse(store, owner, OpenRefusal::Failed, above),
        },
        Stage::Opening | Stage::Pending | Stage::Waiting { .. } => {
            open::refuse(store, store.directory.owner, OpenRefusal::Failed, above);
        }
    }
}
