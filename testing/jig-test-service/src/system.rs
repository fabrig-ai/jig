//! The test application's fault-free system, answered in memory (service.md, 7).
use alloc::boxed::Box;
use jig_test_connector::{
    ApplyResult, Effect, Fact, Form, Key, Looked, Origin, Path, Recovery, SystemEvent, SystemRequest,
};
use skein_lib::Map;

#[derive(Clone, Copy, Debug)]
pub struct Value {
    pub state: u64,
    pub owner: Option<Key>,
}

#[derive(Debug)]
pub struct System {
    values: Map<Path, Value>,
    keyed: Map<Key, u64>,
}

impl System {
    pub(crate) fn paths(&self) -> Box<[Path]> {
        let mut paths = skein_lib::List::with_capacity(self.values.len());
        for (path, _) in &self.values {
            paths.push(path.clone()).expect("system resource count");
        }
        paths.into_boxed()
    }
    #[must_use]
    pub fn new(resources: u32, made: u32) -> Self {
        Self { values: Map::with_capacity(resources), keyed: Map::with_capacity(made) }
    }

    pub fn seed(&mut self, resource: Path, state: u64) -> Result<(), Path> {
        if !self.values.contains_key(&resource) && self.values.len() == self.values.capacity() {
            return Err(resource);
        }
        let _previous = self.values.insert(resource, Value { state, owner: None }).expect("checked system room");
        Ok(())
    }

    pub fn answer(&mut self, request: SystemRequest) -> SystemEvent {
        match request {
            SystemRequest::ReadFact { resource, observed } => {
                let value = self.values.get(&resource);
                let mut state = None;
                if let Some(value) = value {
                    state = Some(value.state);
                }
                let origin = match value {
                    Some(Value { owner: Some(_), .. }) => Origin::Own,
                    Some(Value { owner: None, .. }) | None => Origin::Other,
                };
                SystemEvent::Fact { resource, fact: Fact { state, observed, pending: false }, origin }
            }
            SystemRequest::Apply { entry, attempt, key, effect, form, recovery } => {
                let result = self.apply(key, &effect, form, recovery);
                SystemEvent::Applied { entry, attempt, result }
            }
            SystemRequest::Look { entry, key, effect, recovery } => {
                let value = match effect.resources.first() {
                    Some(path) => self.values.get(path),
                    None => None,
                };
                let (state, owner) = match value {
                    Some(value) => (Some(value.state), value.owner),
                    None => (None, None),
                };
                let looked =
                    Looked { state, owner, found_key: recovery == Recovery::Keyed && self.keyed.contains_key(&key) };
                SystemEvent::Looked { entry, looked }
            }
        }
    }

    fn apply(&mut self, key: Key, effect: &Effect, form: Form, recovery: Recovery) -> ApplyResult {
        if recovery == Recovery::Keyed
            && let Some(state) = self.keyed.get(&key)
        {
            return ApplyResult::Made { state: *state };
        }
        let mut missing = 0_u32;
        for (index, path) in effect.resources.iter().enumerate() {
            let old = self.values.get(path);
            if let Some(expected) = effect.condition {
                let state = match old {
                    Some(value) => value.state,
                    None => 0,
                };
                if state != expected {
                    return ApplyResult::Conflict;
                }
            }
            if form == Form::Creation && old.is_some() {
                return ApplyResult::Conflict;
            }
            if old.is_none() && !effect.resources.get(..index).expect("resource prefix").contains(path) {
                missing = missing.checked_add(1).expect("bounded effect resources");
            }
        }
        if missing > self.values.capacity().checked_sub(self.values.len()).expect("bounded system")
            || (recovery == Recovery::Keyed && self.keyed.len() == self.keyed.capacity())
        {
            return ApplyResult::Transient;
        }
        for path in &effect.resources {
            let _previous = self
                .values
                .insert(path.clone(), Value { state: effect.target, owner: Some(key) })
                .expect("whole effect reserved before applying");
        }
        if recovery == Recovery::Keyed {
            let _previous = self.keyed.insert(key, effect.target).expect("key room reserved before applying");
        }
        ApplyResult::Made { state: effect.target }
    }
}
