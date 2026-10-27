//! The switch gate reads the child's completed check (domain/engine.md, 6).
use super::*;

fn at_switch() -> (Core, Limits) {
    let (mut core, limits) = core();
    let _requests =
        step(&mut core, &Env { now: Time::ZERO, wall: Wall::EPOCH, limits }, Event::Account(accounts::Event::Want));
    core.restart = restart::Restart::Waiting(RestartStep::Switch);
    (core, limits)
}

#[test]
fn a_switch_without_the_childs_check_refuses_the_start() {
    let (mut core, _) = at_switch();
    assert!(!core.restart_checked());
    assert_eq!(core.restart_failure(), Some(RestartStep::Switch));
}

#[test]
fn a_switch_after_the_childs_check_admits_the_start() {
    let (mut core, limits) = at_switch();
    let _requests =
        step(&mut core, &Env { now: Time::ZERO, wall: Wall::EPOCH, limits }, Event::Tasks(tasks::Event::Check));
    assert!(core.restart_checked());
    assert_eq!(core.restart_step(), Some(RestartStep::Switch));
    assert_eq!(core.restore_failure(), None);
}

#[test]
fn checking_at_another_restart_step_refuses_that_step() {
    let (mut core, limits) = at_switch();
    let _requests =
        step(&mut core, &Env { now: Time::ZERO, wall: Wall::EPOCH, limits }, Event::Tasks(tasks::Event::Check));
    core.restart = restart::Restart::Waiting(RestartStep::AdoptRuns);
    assert!(!core.restart_checked());
    assert_eq!(core.restart_failure(), Some(RestartStep::AdoptRuns));
}

const PERSON: u64 = 1;
const MISSING_PERSON: u64 = 2;
const TASK: u64 = 1;
const MISSING_TASK: u64 = 2;
const MISSING_PROJECT: u32 = 2;
const ORPHAN_SOURCE: u32 = 3;
const MISSING_ACCOUNT: u32 = 4;

fn checked(core: &mut Core, limits: &Limits) {
    let _requests =
        step(core, &Env { now: Time::ZERO, wall: Wall::EPOCH, limits: *limits }, Event::Tasks(tasks::Event::Check));
}

fn unresolved(core: &Core, child: RestoreChild, records: u32) {
    assert_eq!(core.restart_failure(), Some(RestartStep::Switch));
    assert_eq!(core.restore_failure(), Some(RestartRequest::Unresolved { child, records }));
}

fn orphan_source() -> accounts::Stored {
    accounts::Stored::Source {
        account: MISSING_ACCOUNT,
        number: ORPHAN_SOURCE,
        place: 0,
        kind: accounts::Kind::Key,
        name: Box::new([]),
        held: None,
    }
}

fn with_orphan_source() -> (Core, Limits) {
    let (mut core, limits) = at_switch();
    core.accounts = accounts::Domain::new(&limits.accounts, core.settings.llm_providers.clone());
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits };
    let _requests = step(&mut core, &env, Event::Account(accounts::Event::Restore { record: orphan_source() }));
    let _requests = step(&mut core, &env, Event::Account(accounts::Event::Want));
    checked(&mut core, &limits);
    (core, limits)
}

#[test]
fn accounts_left_over_a_pending_generation_refuse_the_switch() {
    let (mut core, _) = with_orphan_source();
    core.restart_pending = true;
    assert!(!core.restart_checked());
    unresolved(&core, RestoreChild::Accounts, 1);
    assert_eq!(core.restart_done(RestartStep::Switch), core.restore_failure().unwrap());
}

#[test]
fn accounts_left_with_nothing_pending_admit_the_switch() {
    let (mut core, _) = with_orphan_source();
    assert!(core.restart_checked());
    assert_eq!(core.accounts.unrestored(), 1);
}

#[test]
fn the_switch_requires_accounts_to_finish_restoring() {
    let (mut core, limits) = at_switch();
    core.accounts = accounts::Domain::new(&limits.accounts, core.settings.llm_providers.clone());
    checked(&mut core, &limits);
    assert!(!core.restart_checked());
    assert_eq!(core.restart_failure(), Some(RestartStep::Switch));
}

fn live() -> (Core, Limits, tasks::TaskRecord) {
    let (mut core, limits) = at_switch();
    make_task(&mut core, &limits, TASK);
    let row = core.tasks.task(TASK).expect("fixture live task").clone();
    (core, limits, row)
}

#[test]
fn a_task_project_held_by_the_restart_resolves() {
    let (core, _, row) = live();
    assert!(core.task_references_resolve(&row));
}

#[test]
fn a_task_project_absent_from_the_restart_does_not_resolve() {
    let (core, _, mut row) = live();
    row.project = MISSING_PROJECT;
    assert!(!core.task_references_resolve(&row));
}

#[derive(Clone, Copy, Debug)]
enum PersonField {
    Requester,
    Funder,
    Addressed,
    Taken,
    Answered,
    Grace,
    Added,
    Inbox,
    Cancelled,
    HeldCancellation,
    EscalationHolder,
    EscalationDecider,
    ProposalHolder,
}
const PERSON_FIELDS: &[PersonField] = &[
    PersonField::Requester,
    PersonField::Funder,
    PersonField::Addressed,
    PersonField::Taken,
    PersonField::Answered,
    PersonField::Grace,
    PersonField::Added,
    PersonField::Inbox,
    PersonField::Cancelled,
    PersonField::HeldCancellation,
    PersonField::EscalationHolder,
    PersonField::EscalationDecider,
    PersonField::ProposalHolder,
];

fn naming(row: &mut tasks::TaskRecord, field: PersonField, person: u64) {
    let party = tasks::Party::Person(person);
    let closing = tasks::Closing {
        stage: tasks::Stage::Effects,
        ending: tasks::Ending::Cancelled { by: party, at: Wall::EPOCH, reason: Box::new([]), result: None },
    };
    match field {
        PersonField::Requester => row.requester = party,
        PersonField::Funder => row.funder = tasks::Funder::Pool { project: row.project, person, period: 1 },
        PersonField::Addressed => row.executor = tasks::Executor::Person(tasks::PersonAddress::Person(person)),
        PersonField::Taken => row.taken_by = Some(person),
        PersonField::Answered => row.person_answer = Some(tasks::PersonAnswer { by: person, at: Wall::EPOCH }),
        PersonField::Grace => {
            row.grace = Some(tasks::Grace { by: party, until: Wall::EPOCH, reason: Box::new([]), message: 1 });
        }
        PersonField::Added => row.added = Box::new([tasks::Added { task: MISSING_TASK, by: party, at: Wall::EPOCH }]),
        PersonField::Inbox => {
            row.inbox = Box::new([tasks::Word {
                number: 1,
                from: party,
                kind: tasks::MessageKind::Words,
                words: Box::new([]),
                at: Wall::EPOCH,
                hits: 1,
                eligible: true,
            }]);
        }
        PersonField::Cancelled => row.phase = tasks::Phase::Closing(closing),
        PersonField::HeldCancellation => {
            row.phase =
                tasks::Phase::Held { was: tasks::Was::Closing(closing), why: tasks::Hold::Stopped, source: None }
        }
        PersonField::EscalationHolder => {
            row.escalation = tasks::Escalation::Waiting {
                revision: 1,
                holder: tasks::EscalationHolder::Person(person),
                entry: 1,
                since: Wall::EPOCH,
            }
        }
        PersonField::EscalationDecider => {
            row.escalation = tasks::Escalation::Rejected { revision: 1, by: person, reason: Box::new([]) }
        }
        PersonField::ProposalHolder => {
            row.proposal = Some(Box::new(tasks::Proposal {
                number: 1,
                proposer: TASK,
                project: row.project,
                action: tasks::ProposalAction::Release { task: TASK },
                reason: Box::new([]),
                as_holder: false,
                state: tasks::ProposalState::Pending {
                    holder: tasks::ProposalHolder::Person(person),
                    since: Wall::EPOCH,
                },
            }));
        }
    }
}

#[test]
fn every_person_field_of_a_live_task_resolves_a_held_person() {
    let (core, _, template) = live();
    for field in PERSON_FIELDS {
        let mut row = template.clone();
        naming(&mut row, *field, PERSON);
        assert!(core.task_references_resolve(&row), "{field:?}");
    }
}

#[test]
fn every_person_field_of_a_live_task_refuses_an_absent_person() {
    let (core, _, template) = live();
    for field in PERSON_FIELDS {
        let mut row = template.clone();
        naming(&mut row, *field, MISSING_PERSON);
        assert!(!core.task_references_resolve(&row), "{field:?}");
    }
}

fn read_position(core: &mut Core, limits: &Limits, task: u64) {
    let identity = core.people.identity(PERSON).unwrap().clone();
    core.people = people::Domain::new(&limits.people, Box::new([]), core.settings.deployment_provider, Box::new([]));
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits.people };
    let mut out = Queue::with_capacity(people::max_out(&limits.people));
    for record in [
        people::Stored::Person { number: PERSON, identity },
        people::Stored::Roles {
            project: 1,
            holdings: Box::new([people::Holding { person: PERSON, role: people::Role::Owner }]),
        },
        people::Stored::ReadConversation { party: PERSON, task, through: 1 },
    ] {
        people::step(&mut core.people, &env, people::Event::Restore { record }, &mut out);
    }
    people::step(&mut core.people, &env, people::Event::Restored, &mut out);
}

fn cold_tasks(core: &mut Core, limits: &Limits) {
    let mut restored = fixtures::fresh_tasks(core, limits);
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits.tasks };
    let mut out = Queue::with_capacity(tasks::max_out(&limits.tasks));
    for (_, row) in core.tasks.funding_records() {
        tasks::step(&mut restored, &env, tasks::Event::Restore { record: tasks::Stored::Ledger(*row) }, &mut out);
    }
    for view in core.tasks.view_tasks() {
        let row = core.tasks.task(view.number).unwrap().clone();
        tasks::step(
            &mut restored,
            &env,
            tasks::Event::Restore { record: tasks::Stored::Live(Box::new(row)) },
            &mut out,
        );
    }
    core.tasks = restored;
    checked(core, limits);
}

#[test]
fn a_read_position_over_a_restored_live_task_admits_the_switch() {
    let (mut core, limits, _) = live();
    read_position(&mut core, &limits, TASK);
    cold_tasks(&mut core, &limits);
    assert!(core.restart_checked());
}

#[test]
fn a_read_position_over_no_restored_task_refuses_the_switch() {
    let (mut core, limits) = at_switch();
    read_position(&mut core, &limits, MISSING_TASK);
    checked(&mut core, &limits);
    assert!(!core.restart_checked());
    unresolved(&core, RestoreChild::People, 1);
}

#[test]
fn a_read_position_resolves_when_the_census_holds_its_ended_task() {
    let (mut core, limits, mut row) = live();
    core.tasks = fixtures::fresh_tasks(&core, &limits);
    row.phase = tasks::Phase::Ended(tasks::Ending::Done(tasks::TaskResult::Report { words: Box::new([]) }));
    row.ended_at = Some(Wall::EPOCH);
    row.result_position = 1;
    core.counters.deployment.tasks = TASK;
    core.counters.deployment.messages = 1;
    read_position(&mut core, &limits, TASK);
    core.restart = restart::Restart::Waiting(RestartStep::LoadInbox);
    let _census = core.census(&limits);
    let restored = core.restore_inbox_row(&limits, &Record::Tasks(tasks::Stored::Ended(Box::new(row))));
    core.restart = restart::Restart::Waiting(RestartStep::Switch);
    checked(&mut core, &limits);
    assert!(restored);
    assert!(core.restart_checked());
    assert!(
        core.people.read_conversations().is_empty(),
        "ended positions leave the live restore even if the archive cache evicts their owner"
    );
}

#[test]
fn accounts_removed_only_at_finish_refuse_a_pending_generation() {
    let (mut core, limits) = at_switch();
    let provider = core.settings.llm_providers.providers[0].number;
    core.accounts = accounts::Domain::new(&limits.accounts, core.settings.llm_providers.clone());
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits };
    let _requests = step(
        &mut core,
        &env,
        Event::Account(accounts::Event::Restore {
            record: accounts::Stored::Account {
                number: 0,
                provider,
                holder: accounts::Holder::Deployment,
                place: 0,
                uses: None,
                subject: Box::new([]),
                display: Box::new([]),
            },
        }),
    );
    let _requests = step(&mut core, &env, Event::Account(accounts::Event::Want));
    core.restart_pending = true;
    checked(&mut core, &limits);
    assert!(!core.restart_checked());
    unresolved(&core, RestoreChild::Accounts, 1);
    assert_eq!(core.accounts.accounts(), 0);
}

#[test]
fn a_pending_generation_with_every_account_restored_admits_the_switch() {
    let (mut core, limits) = at_switch();
    core.restart_pending = true;
    checked(&mut core, &limits);
    assert!(core.restart_checked());
    assert_eq!(core.restore_failure(), None);
}

#[test]
fn a_live_tasks_placed_hold_resolves_its_party_owner_even_beside_another_cause() {
    for person in [PERSON, MISSING_PERSON] {
        let (mut core, limits, _) = live();
        send_task(&mut core, &limits, placed(TASK, person, 71));
        let mut row = core.tasks.task(TASK).unwrap().clone();
        assert_eq!(row.placed, Some(71));
        assert_eq!(core.task_references_resolve(&row), person == PERSON);
        row.phase =
            tasks::Phase::Held { was: tasks::Was::Active(tasks::Active::Due), why: tasks::Hold::Budget, source: None };
        assert_eq!(core.task_references_resolve(&row), person == PERSON);
    }
}

#[test]
fn a_placed_hold_masked_by_another_still_resolves_its_owner_before_switch() {
    let (mut core, mut limits) = at_switch();
    limits.tasks.placed_holds = 2;
    limits.tasks.project_placed_holds = 2;
    limits.tasks.party_placed_holds = 2;
    core.tasks = fixtures::fresh_tasks(&core, &limits);
    make_task(&mut core, &limits, TASK);
    send_task(&mut core, &limits, placed(TASK, PERSON, 71));
    send_task(&mut core, &limits, placed(TASK, MISSING_PERSON, 72));
    assert_eq!(core.tasks.task(TASK).unwrap().placed, Some(71));
    assert_eq!(core.tasks.next_placed_hold(71).unwrap().number, 72);
    checked(&mut core, &limits);
    assert!(!core.restart_checked());
    unresolved(&core, RestoreChild::Tasks, 1);
}
