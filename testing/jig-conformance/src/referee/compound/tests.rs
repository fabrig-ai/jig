use super::*;
use crate::referee::{Observed, Phase, Policy, Referee, Scope, Source};

const AT: u64 = 0;

fn task(number: u64) -> Task {
    Task {
        settings: crate::referee::SettingsRevision::default(),
        spec_revision: 0,
        hold_revision: 0,
        placed: None,
        placed_reason: false,
        tracked: false,
        number,
        project: 1,
        source: Source::Deployment,
        scope: Scope::default(),
        executor: (0, 1),
        budget: 0,
        spent: 0,
        spent_below: 0,
        reserved: 0,
        run_reserved: 0,
        run_spent: 0,
        attempt: 0,
        period: None,
        phase: Phase::Waiting,
        dependencies: vec![],
        delegates: vec![],
        pools: vec![],
        inbox: vec![],
        proposal: None,
    }
}
fn referee() -> (Referee, Snapshot) {
    let mut referee = Referee::new(Policy {
        projections: BTreeMap::new(),
        deployment: Scope::default(),
        projects: BTreeMap::from([(1, Scope::default())]),
        people: BTreeMap::new(),
        effect_accepters: BTreeSet::new(),
        requirements: vec![],
        implications: BTreeSet::new(),
        kinds: BTreeMap::new(),
        owned: vec![],
        participating: BTreeSet::new(),
        mechanics: BTreeSet::new(),
        delivery_bound: 20,
        uncertainty_bound: 30,
        story_steps: 100,
    });
    let snapshot = Snapshot { tasks: BTreeMap::from([(1, task(1)), (2, task(2))]), ..Snapshot::default() };
    referee.observe(AT, Observed::Durable { number: 1, snapshot: snapshot.clone() }).expect("baseline tasks");
    referee.observe(AT, Observed::CompoundAsked { tasks: vec![1, 2] }).expect("independent ask");
    (referee, snapshot)
}
#[test]
fn a_compound_made_across_two_commits_breaks_once() {
    let (mut referee, mut snapshot) = referee();
    snapshot.tasks.get_mut(&1).expect("first task").tracked = true;
    referee.observe(AT, Observed::Durable { number: 2, snapshot: snapshot.clone() }).expect("first change");
    snapshot.tasks.get_mut(&2).expect("second task").tracked = true;
    referee.observe(AT, Observed::Durable { number: 3, snapshot }).expect("second change");
    assert_eq!(
        referee.observe(AT, Observed::Compound { tasks: vec![1, 2], made: true }).expect_err("split compound").promise,
        Promise::Once
    );
}
#[test]
fn a_refused_compound_with_a_changed_task_breaks_once() {
    let (mut referee, mut snapshot) = referee();
    snapshot.tasks.get_mut(&1).expect("first task").tracked = true;
    referee.observe(AT, Observed::Durable { number: 2, snapshot }).expect("changed task");
    assert_eq!(
        referee.observe(AT, Observed::Compound { tasks: vec![1, 2], made: false }).expect_err("refused change").promise,
        Promise::Once
    );
}
#[test]
fn a_compound_retry_keeps_its_first_atomic_change_despite_later_commits() {
    let (mut referee, mut snapshot) = referee();
    for task in snapshot.tasks.values_mut() {
        task.tracked = true;
    }
    referee.observe(AT, Observed::Durable { number: 2, snapshot: snapshot.clone() }).expect("whole compound");
    referee.observe(AT, Observed::Compound { tasks: vec![1, 2], made: true }).expect("made answer");
    snapshot.tasks.get_mut(&1).expect("first task").phase = Phase::Active;
    referee.observe(AT, Observed::Durable { number: 3, snapshot }).expect("later independent change");
    referee.observe(AT, Observed::CompoundAsked { tasks: vec![1, 2] }).expect("retry ask");
    referee.observe(AT, Observed::Compound { tasks: vec![1, 2], made: true }).expect("same original compound");
}
#[test]
fn a_compound_answer_without_an_ask_breaks_once() {
    let (mut referee, _) = referee();
    assert_eq!(
        referee.observe(AT, Observed::Compound { tasks: vec![9], made: false }).expect_err("unasked answer").promise,
        Promise::Once
    );
}
#[test]
fn a_refused_compound_leaves_every_named_row_unchanged() {
    let (mut referee, snapshot) = referee();
    referee.observe(AT, Observed::Durable { number: 2, snapshot }).expect("unchanged tasks");
    referee.observe(AT, Observed::Compound { tasks: vec![1, 2], made: false }).expect("refusal changed nothing");
}

#[test]
fn a_root_release_names_only_its_placed_subtree_and_preserves_other_causes() {
    use crate::referee::{PlacedHold, Revision};
    let mut before = Snapshot::default();
    for number in [1, 2, 7] {
        let mut row = task(number);
        row.phase = Phase::Held;
        row.hold_revision = 3;
        row.placed = Some(100);
        row.placed_reason = true;
        if number == 2 {
            row.source = Source::Task(1);
        }
        before.tasks.insert(number, row);
    }
    before.placed_holds.insert(100, PlacedHold { root: Some(1), lifted: false });
    let mut after = before.clone();
    after.placed_holds.remove(&100);
    for number in [1, 2] {
        after.tasks.get_mut(&number).unwrap().phase = Phase::Active;
        after.tasks.get_mut(&number).unwrap().placed = None;
    }
    let mut revisions = super::super::revisions::Revisions::default();
    revisions.named(1, Revision::Hold(3));
    assert_eq!(revisions.durable(&before, &after), Ok(()));
    for fault in 0..4 {
        let mut before = before.clone();
        let mut after = after.clone();
        match fault {
            0 => {
                after.tasks.get_mut(&7).unwrap().phase = Phase::Active;
            }
            1 => {
                before.tasks.get_mut(&2).unwrap().placed_reason = false;
            }
            2 => {
                after.placed_holds.insert(100, PlacedHold { root: Some(1), lifted: false });
            }
            3 => {}
            _ => unreachable!(),
        }
        let mut revisions = super::super::revisions::Revisions::default();
        revisions.named(1, Revision::Hold(if fault == 3 { 2 } else { 3 }));
        assert!(revisions.durable(&before, &after).is_err(), "fault {fault}");
    }
}
