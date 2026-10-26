use super::*;

pub(super) fn assigned(budget: u64) -> Observed {
    Observed::Assigned { task: 1, attempt: 1, conversation: 1, resumed: false, budget, settings: 0 }
}

pub(super) fn pattern() -> Pattern {
    Pattern { base: vec![], terminal: b"owned".to_vec(), open: true }
}

pub(super) fn name() -> Name {
    Name { connector: 1, path: vec![b"owned".to_vec(), b"item".to_vec()] }
}

pub(super) fn scope() -> Scope {
    Scope {
        budget: 100,
        tools: 15,
        grants: vec![Grant { connector: 1, kind: 5, pattern: pattern() }],
        executors: BTreeSet::from([(0, 1)]),
        descendants: 3,
        depth: 2,
        notes: 7,
        ..Scope::default()
    }
}

pub(super) fn policy(recovery: Recovery) -> Policy {
    Policy {
        projections: BTreeMap::new(),
        deployment: scope(),
        projects: BTreeMap::from([(1, scope())]),
        people: BTreeMap::from([((1, 1), scope())]),
        effect_accepters: BTreeSet::from([(1, 1)]),
        requirements: vec![],
        implications: BTreeSet::new(),
        kinds: BTreeMap::from([((1, 5), Kind { recovery, price: 3 })]),
        owned: vec![(1, pattern())],
        participating: BTreeSet::new(),
        mechanics: BTreeSet::new(),
        delivery_bound: 20,
        uncertainty_bound: 30,
        story_steps: 10,
    }
}

pub(super) fn task(number: u64) -> Task {
    Task {
        settings: SettingsRevision { task: 0, run: Some(0) },
        tracked: false,
        number,
        project: 1,
        source: Source::Person(1),
        scope: scope(),
        executor: (0, 1),
        budget: 100,
        spent: 3,
        spent_below: 0,
        reserved: 10,
        run_reserved: 10,
        run_spent: 0,
        attempt: 1,
        period: None,
        phase: Phase::Active,
        spec_revision: 0,
        hold_revision: 0,
        dependencies: vec![],
        delegates: vec![],
        pools: vec![],
        inbox: vec![],
        proposal: None,
    }
}

pub(super) fn key() -> SystemKey {
    SystemKey { deployment: [31; 16], task: 1, attempt: 1, completion: 1, position: 0, purpose: 9 }
}

pub(super) fn decision() -> Decision {
    Decision {
        projection: false,
        connector: 1,
        key: key(),
        kind: 5,
        resources: vec![name()],
        condition: None,
        target: 13,
        judged_state: 13,
        accepted_by: None,
    }
}

pub(super) fn snapshot() -> Snapshot {
    Snapshot {
        tasks: BTreeMap::from([(1, task(1))]),
        receipts: BTreeSet::from([Receipt::Task(1), Receipt::Claim(1, 1), Receipt::Outbox(1, key(), 1)]),
        decisions: vec![decision()],
        ..Snapshot::default()
    }
}

pub(super) fn effect() -> Effect {
    Effect {
        key: key(),
        kind: 5,
        resources: vec![SystemName(name().path)],
        condition: None,
        target: 13,
        before: vec![ValueObserved { state: None, owner: None }],
        applied: true,
        copy: Copy::First,
    }
}

/// The misbehaving fake supplies observations through exactly the normal API.
pub(super) struct Fake {
    pub(super) referee: Referee,
    pub(super) now: u64,
}

impl Fake {
    pub(super) fn new(recovery: Recovery) -> Self {
        Self { referee: Referee::new(policy(recovery)), now: 1 }
    }
    pub(super) fn send(&mut self, event: Observed) -> Result<(), Violation> {
        self.referee.observe(self.now, event)
    }
    pub(super) fn commit(&mut self, number: u64, snapshot: Snapshot) -> Result<(), Violation> {
        self.send(Observed::Durable { number, snapshot })
    }
    pub(super) fn ready(recovery: Recovery) -> Self {
        let mut fake = Self::new(recovery);
        fake.commit(1, snapshot()).expect("authorized initial decision");
        fake
    }
    pub(super) fn caught(&mut self, event: Observed, promise: Promise) {
        let failure = self.send(event).expect_err("the misbehaving fake must be caught");
        assert_eq!(failure.promise, promise, "{failure:?}");
    }
}
