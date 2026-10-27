//! Field-for-field document values (client/domain/contract.md, 5–7).
use super::{
    Limits, List, Reader, Wall, c, charge, chip_piece, owner_account_state_fields, owner_host_piece,
    owner_host_run_piece, piece_count, store_fill_fields, too_large, w,
};
#[expect(clippy::manual_map, reason = "step code matches optional fields without closures")]
pub(crate) fn owner_account_fields(
    value: &w::OwnerAccount,
    limits: &Limits,
    room: &mut u64,
) -> Result<c::OwnerAccount, w::Problem> {
    super::charge_account(value.uses().as_ref(), value.display(), room)?;
    // An account's runs and its waiting tasks stay lists bounded by the rows.
    let chips = List::<c::Chip>::worst_case(limits.client.rows);
    charge(room, chips)?;
    charge(room, chips)?;
    let opt0 = match value.generation() {
        Some(item) => Some(*item),
        None => None,
    };
    let opt1 = match value.spent_until() {
        Some(item) => Some(Wall::from_nanos(*item)),
        None => None,
    };
    Ok(c::OwnerAccount {
        number: value.number(),
        place: value.place(),
        uses: super::owner_account_uses(value.uses().as_ref()),
        plan: value.plan(),
        provider: *value.provider(),
        provider_name: match value.provider_name() {
            Some(name) => Some(crate::translate::facts::document_text(name, limits)?),
            None => None,
        },
        manage_usage: super::checked_manage_usage(value.manage_usage().as_deref(), w::Path::OwnerAccountManageUsage)?,
        display: crate::translate::facts::document_text(value.display(), limits)?,
        holder: super::owner_account_holder(value.holder()),
        sources: super::owner_account_source_piece(value.sources(), limits, room)?,
        generation: opt0,
        state: owner_account_state_fields(value.state(), limits),
        usable: value.usable(),
        expiring: value.expiring(),
        attention: value.attention(),
        spent_until: opt1,
        runs: chip_piece(value.runs(), limits, limits.client.rows)?,
        waiting: chip_piece(value.waiting(), limits, limits.client.rows)?,
        cut: value.cut(),
    })
}

pub(crate) fn owner_account_piece(
    bytes: &[u8],
    limits: &Limits,
    room: &mut u64,
) -> Result<List<c::OwnerAccount>, w::Problem> {
    let count = piece_count(bytes)?;
    if count > limits.client.accounts.accounts {
        return Err(too_large());
    }
    charge(room, List::<c::OwnerAccount>::worst_case(count))?;
    let mut reader = Reader::new(bytes);
    let mut rows = List::with_capacity(count);
    for _ in 0..count {
        let Ok(Some(item)) = skein_codec::piece::split(&mut reader) else { unreachable!("counted pieces split") };
        let row = w::OwnerAccount::decode(&limits.wire, &mut Reader::new(&item))?;
        rows.push(owner_account_fields(&row, limits, room)?).expect("counted pieces fit");
    }
    Ok(rows)
}

pub(crate) fn owner_capacity_fields(value: &w::OwnerCapacity, _limits: &Limits) -> c::OwnerCapacity {
    c::OwnerCapacity { used: value.used(), limit: value.limit() }
}

pub(crate) fn owner_project_capacity_fields(
    value: &w::OwnerProjectCapacity,
    limits: &Limits,
) -> c::OwnerProjectCapacity {
    c::OwnerProjectCapacity {
        project: value.project(),
        tasks: owner_capacity_fields(value.tasks(), limits),
        proposals: value.proposals(),
    }
}

pub(crate) fn owner_agent_shortfall_fields(value: &w::OwnerAgentShortfall, _limits: &Limits) -> c::OwnerAgentShortfall {
    match value {
        w::OwnerAgentShortfall::Model(row) => {
            c::OwnerAgentShortfall::Model { charter: row.charter(), model: row.model() }
        }
        w::OwnerAgentShortfall::Window(row) => {
            c::OwnerAgentShortfall::Window { charter: row.charter(), model: row.model() }
        }
        w::OwnerAgentShortfall::Output(row) => {
            c::OwnerAgentShortfall::Output { charter: row.charter(), model: row.model() }
        }
        w::OwnerAgentShortfall::Turns => c::OwnerAgentShortfall::Turns,
        w::OwnerAgentShortfall::Spend => c::OwnerAgentShortfall::Spend,
        w::OwnerAgentShortfall::Time => c::OwnerAgentShortfall::Time,
        w::OwnerAgentShortfall::Waiting => c::OwnerAgentShortfall::Waiting,
        w::OwnerAgentShortfall::Checks => c::OwnerAgentShortfall::Checks,
        w::OwnerAgentShortfall::Message => c::OwnerAgentShortfall::Message,
        w::OwnerAgentShortfall::Inbox => c::OwnerAgentShortfall::Inbox,
        w::OwnerAgentShortfall::Unacknowledged => c::OwnerAgentShortfall::Unacknowledged,
    }
}

pub(crate) fn owner_hello_refusal_fields(value: &w::OwnerHelloRefusal, limits: &Limits) -> c::OwnerHelloRefusal {
    match value {
        w::OwnerHelloRefusal::Agent(row) => {
            c::OwnerHelloRefusal::Agent { why: owner_agent_shortfall_fields(row.why(), limits) }
        }
        w::OwnerHelloRefusal::ReservedChannel => c::OwnerHelloRefusal::ReservedChannel,
        w::OwnerHelloRefusal::Workers(row) => c::OwnerHelloRefusal::Workers { limit: row.limit() },
        w::OwnerHelloRefusal::Position(row) => c::OwnerHelloRefusal::Position { worker: row.worker() },
        w::OwnerHelloRefusal::StopBound(row) => c::OwnerHelloRefusal::StopBound { grace: row.grace() },
    }
}

pub(crate) fn owner_hello_refusal_line_fields(
    value: &w::OwnerHelloRefusalLine,
    limits: &Limits,
) -> c::OwnerHelloRefusalLine {
    c::OwnerHelloRefusalLine {
        channel: value.channel(),
        at: Wall::from_nanos(value.at()),
        slots: value.slots(),
        stop_bound: value.stop_bound(),
        why: owner_hello_refusal_fields(value.why(), limits),
    }
}

pub(crate) fn owner_request_key_fields(value: &w::OwnerRequestKey, _limits: &Limits) -> c::OwnerRequestKey {
    c::OwnerRequestKey { person: value.person(), key: *value.key() }
}

pub(crate) fn owner_person_capacity_action_fields(
    value: &w::OwnerPersonCapacityAction,
    limits: &Limits,
) -> c::OwnerPersonCapacityAction {
    match value {
        w::OwnerPersonCapacityAction::AccountSignIn => c::OwnerPersonCapacityAction::AccountSignIn,
        w::OwnerPersonCapacityAction::AccountSignOut => c::OwnerPersonCapacityAction::AccountSignOut,
        w::OwnerPersonCapacityAction::AccountChange => c::OwnerPersonCapacityAction::AccountChange,
        w::OwnerPersonCapacityAction::Create => c::OwnerPersonCapacityAction::Create,
        w::OwnerPersonCapacityAction::Compound => c::OwnerPersonCapacityAction::Compound,
        w::OwnerPersonCapacityAction::Application(row) => {
            c::OwnerPersonCapacityAction::Application { connector: row.connector() }
        }
        w::OwnerPersonCapacityAction::Note(row) => c::OwnerPersonCapacityAction::Note { name: row.name() },
        w::OwnerPersonCapacityAction::Service => c::OwnerPersonCapacityAction::Service,
        w::OwnerPersonCapacityAction::Adoption => c::OwnerPersonCapacityAction::Adoption,
        w::OwnerPersonCapacityAction::Roles => c::OwnerPersonCapacityAction::Roles,
        w::OwnerPersonCapacityAction::Policy => c::OwnerPersonCapacityAction::Policy,
        w::OwnerPersonCapacityAction::Pool(row) => c::OwnerPersonCapacityAction::Pool { person: row.person() },
        w::OwnerPersonCapacityAction::Escalation(row) => c::OwnerPersonCapacityAction::Escalation { task: row.task() },
        w::OwnerPersonCapacityAction::Proposal(row) => c::OwnerPersonCapacityAction::Proposal {
            proposer: crate::translate::answers::proposer(row.proposer(), limits),
            number: row.number(),
        },
        w::OwnerPersonCapacityAction::Words(row) => c::OwnerPersonCapacityAction::Words { task: row.task() },
        w::OwnerPersonCapacityAction::Question(row) => {
            c::OwnerPersonCapacityAction::Question { task: row.task(), question: row.question() }
        }
        w::OwnerPersonCapacityAction::Amend(row) => c::OwnerPersonCapacityAction::Amend { task: row.task() },
        w::OwnerPersonCapacityAction::MoveTask(row) => c::OwnerPersonCapacityAction::Move { task: row.task() },
        w::OwnerPersonCapacityAction::TakePerson(row) => c::OwnerPersonCapacityAction::TakePerson { task: row.task() },
        w::OwnerPersonCapacityAction::HandBackPerson(row) => {
            c::OwnerPersonCapacityAction::HandBackPerson { task: row.task() }
        }
        w::OwnerPersonCapacityAction::AnswerPerson(row) => {
            c::OwnerPersonCapacityAction::AnswerPerson { task: row.task() }
        }
        w::OwnerPersonCapacityAction::Stop(row) => c::OwnerPersonCapacityAction::Stop { task: row.task() },
        w::OwnerPersonCapacityAction::Cancel(row) => c::OwnerPersonCapacityAction::Cancel { task: row.task() },
        w::OwnerPersonCapacityAction::Release(row) => c::OwnerPersonCapacityAction::Release { task: row.task() },
        w::OwnerPersonCapacityAction::MarkSeen(row) => {
            c::OwnerPersonCapacityAction::MarkSeen { seen: crate::translate::answers::seen(row.seen(), limits) }
        }
    }
}

pub(crate) fn owner_person_capacity_cause_fields(
    value: &w::OwnerPersonCapacityCause,
    limits: &Limits,
) -> c::OwnerPersonCapacityCause {
    match value {
        w::OwnerPersonCapacityCause::Requests(row) => c::OwnerPersonCapacityCause::Requests { limit: row.limit() },
        w::OwnerPersonCapacityCause::Pending(row) => c::OwnerPersonCapacityCause::Pending { limit: row.limit() },
        w::OwnerPersonCapacityCause::Waiters(row) => c::OwnerPersonCapacityCause::Waiters { limit: row.limit() },
        w::OwnerPersonCapacityCause::Limit(row) => c::OwnerPersonCapacityCause::Limit {
            which: crate::translate::answers::admission_limit(row.which(), limits),
        },
    }
}

#[expect(clippy::manual_map, reason = "step code matches optional fields without closures")]
pub(crate) fn owner_person_capacity_refusal_fields(
    value: &w::OwnerPersonCapacityRefusal,
    limits: &Limits,
) -> c::OwnerPersonCapacityRefusal {
    let opt0 = match value.project() {
        Some(item) => Some(*item),
        None => None,
    };
    c::OwnerPersonCapacityRefusal {
        key: owner_request_key_fields(value.key(), limits),
        project: opt0,
        action: owner_person_capacity_action_fields(value.action(), limits),
        at: Wall::from_nanos(value.at()),
        cause: owner_person_capacity_cause_fields(value.cause(), limits),
    }
}

#[expect(clippy::manual_map, reason = "step code matches optional fields without closures")]
pub(crate) fn owner_task_capacity_action_fields(
    value: &w::OwnerTaskCapacityAction,
    limits: &Limits,
) -> c::OwnerTaskCapacityAction {
    match value {
        w::OwnerTaskCapacityAction::Batch(row) => {
            let opt0 = match row.first() {
                Some(item) => Some(*item),
                None => None,
            };
            c::OwnerTaskCapacityAction::Batch { first: opt0, members: row.members() }
        }
        w::OwnerTaskCapacityAction::Proposal(row) => c::OwnerTaskCapacityAction::Proposal { number: row.number() },
        w::OwnerTaskCapacityAction::PersonProposal(row) => {
            c::OwnerTaskCapacityAction::PersonProposal { number: row.number() }
        }
        w::OwnerTaskCapacityAction::Message(row) => {
            c::OwnerTaskCapacityAction::Message { task: row.task(), number: row.number() }
        }
        w::OwnerTaskCapacityAction::Source(row) => {
            c::OwnerTaskCapacityAction::Source { funder: crate::translate::facts::funder(row.funder(), limits) }
        }
        w::OwnerTaskCapacityAction::Introduce(row) => {
            c::OwnerTaskCapacityAction::Introduce { left: row.left(), right: row.right() }
        }
        w::OwnerTaskCapacityAction::Subscription(row) => {
            c::OwnerTaskCapacityAction::Subscription { number: row.number() }
        }
        w::OwnerTaskCapacityAction::Effect(row) => c::OwnerTaskCapacityAction::Effect { entry: row.entry() },
        w::OwnerTaskCapacityAction::Claim(row) => c::OwnerTaskCapacityAction::Claim { attempt: row.attempt() },
        w::OwnerTaskCapacityAction::Place(row) => c::OwnerTaskCapacityAction::Place { hold: row.hold() },
        w::OwnerTaskCapacityAction::Release => c::OwnerTaskCapacityAction::Release,
        w::OwnerTaskCapacityAction::MoveTask => c::OwnerTaskCapacityAction::Move,
    }
}

pub(crate) fn owner_task_capacity_cause_fields(
    value: &w::OwnerTaskCapacityCause,
    _limits: &Limits,
) -> c::OwnerTaskCapacityCause {
    match value {
        w::OwnerTaskCapacityCause::PlacedHolds(row) => c::OwnerTaskCapacityCause::PlacedHolds { limit: row.limit() },
        w::OwnerTaskCapacityCause::ProjectPlacedHolds(row) => {
            c::OwnerTaskCapacityCause::ProjectPlacedHolds { limit: row.limit() }
        }
        w::OwnerTaskCapacityCause::PartyPlacedHolds(row) => {
            c::OwnerTaskCapacityCause::PartyPlacedHolds { limit: row.limit() }
        }
        w::OwnerTaskCapacityCause::Exemptions(row) => c::OwnerTaskCapacityCause::Exemptions { limit: row.limit() },
        w::OwnerTaskCapacityCause::Tasks(row) => c::OwnerTaskCapacityCause::Tasks { limit: row.limit() },
        w::OwnerTaskCapacityCause::CreationFlights(row) => {
            c::OwnerTaskCapacityCause::CreationFlights { limit: row.limit() }
        }
        w::OwnerTaskCapacityCause::ResourceReports(row) => {
            c::OwnerTaskCapacityCause::ResourceReports { limit: row.limit() }
        }
        w::OwnerTaskCapacityCause::ProjectTasks(row) => c::OwnerTaskCapacityCause::ProjectTasks { limit: row.limit() },
        w::OwnerTaskCapacityCause::Tree(row) => c::OwnerTaskCapacityCause::Tree { limit: row.limit() },
        w::OwnerTaskCapacityCause::Delegates(row) => c::OwnerTaskCapacityCause::Delegates { limit: row.limit() },
        w::OwnerTaskCapacityCause::Inbox(row) => {
            c::OwnerTaskCapacityCause::Inbox { task: row.task(), messages: row.messages(), bytes: row.bytes() }
        }
        w::OwnerTaskCapacityCause::Holdings(row) => c::OwnerTaskCapacityCause::Holdings { limit: row.limit() },
        w::OwnerTaskCapacityCause::HoldWaiters(row) => c::OwnerTaskCapacityCause::HoldWaiters { limit: row.limit() },
        w::OwnerTaskCapacityCause::Proposals(row) => c::OwnerTaskCapacityCause::Proposals { limit: row.limit() },
        w::OwnerTaskCapacityCause::References(row) => c::OwnerTaskCapacityCause::References { limit: row.limit() },
        w::OwnerTaskCapacityCause::Subscriptions(row) => {
            c::OwnerTaskCapacityCause::Subscriptions { limit: row.limit() }
        }
        w::OwnerTaskCapacityCause::Funders(row) => c::OwnerTaskCapacityCause::Funders { limit: row.limit() },
        w::OwnerTaskCapacityCause::Writers(row) => c::OwnerTaskCapacityCause::Writers { limit: row.limit() },
    }
}

#[expect(clippy::manual_map, reason = "step code matches optional fields without closures")]
pub(crate) fn owner_task_capacity_request_fields(
    value: &w::OwnerTaskCapacityRequest,
    limits: &Limits,
) -> c::OwnerTaskCapacityRequest {
    let opt0 = match value.project() {
        Some(item) => Some(*item),
        None => None,
    };
    c::OwnerTaskCapacityRequest {
        project: opt0,
        by: crate::translate::facts::owner_requester(value.by(), limits),
        action: owner_task_capacity_action_fields(value.action(), limits),
        cause: owner_task_capacity_cause_fields(value.cause(), limits),
    }
}

pub(crate) fn owner_task_capacity_refusal_fields(
    value: &w::OwnerTaskCapacityRefusal,
    limits: &Limits,
) -> c::OwnerTaskCapacityRefusal {
    c::OwnerTaskCapacityRefusal {
        request: owner_task_capacity_request_fields(value.request(), limits),
        at: Wall::from_nanos(value.at()),
    }
}

pub(crate) fn owner_deployment_fields(
    value: &w::OwnerDeployment,
    limits: &Limits,
) -> Result<c::OwnerDeployment, w::Problem> {
    let mut room = super::document_room(limits);
    charge(&mut room, u64::try_from(size_of::<c::OwnerDeployment>()).ok())?;
    if value.projects().len() > limits.client.rows {
        return Err(too_large());
    }
    let mut list0 = List::with_capacity(value.projects().len());
    for item in value.projects() {
        list0.push(owner_project_capacity_fields(item, limits)).expect("reserved supplied list");
    }
    if value.hello_refusals().len() > limits.client.rows {
        return Err(too_large());
    }
    let mut list1 = List::with_capacity(value.hello_refusals().len());
    for item in value.hello_refusals() {
        list1.push(owner_hello_refusal_line_fields(item, limits)).expect("reserved supplied list");
    }
    if value.person_room_refusals().len() > limits.client.rows {
        return Err(too_large());
    }
    let mut list2 = List::with_capacity(value.person_room_refusals().len());
    for item in value.person_room_refusals() {
        list2.push(owner_person_capacity_refusal_fields(item, limits)).expect("reserved supplied list");
    }
    if value.task_room_refusals().len() > limits.client.rows {
        return Err(too_large());
    }
    let mut list3 = List::with_capacity(value.task_room_refusals().len());
    for item in value.task_room_refusals() {
        list3.push(owner_task_capacity_refusal_fields(item, limits)).expect("reserved supplied list");
    }
    if value.connectors().len() > limits.client.rows {
        return Err(too_large());
    }
    let mut list4 = List::with_capacity(value.connectors().len());
    for item in value.connectors() {
        list4.push(*item).expect("reserved supplied list");
    }
    Ok(c::OwnerDeployment {
        providers: super::catalogue::offers(value.providers(), limits, &mut room)?,
        providers_cut: value.providers_cut(),
        key_bytes: value.key_bytes(),
        readiness: super::catalogue::readiness(value.readiness(), limits, &mut room)?,
        readiness_cut: value.readiness_cut(),
        store: store_fill_fields(value.store(), limits),
        hosts: owner_host_piece(value.hosts(), limits, limits.client.rows, &mut room)?,
        attempts: owner_host_run_piece(value.attempts(), limits, limits.client.rows)?,
        accounts: owner_account_piece(value.accounts(), limits, &mut room)?,
        tasks: owner_capacity_fields(value.tasks(), limits),
        workers: owner_capacity_fields(value.workers(), limits),
        attempts_capacity: owner_capacity_fields(value.attempts_capacity(), limits),
        waiting_attempts: value.waiting_attempts(),
        proposals: owner_capacity_fields(value.proposals(), limits),
        projects: list0,
        grace_bound: value.grace_bound(),
        hello_refusals: list1,
        hosts_forgotten: value.hosts_forgotten(),
        hello_refusals_forgotten: value.hello_refusals_forgotten(),
        person_room_refusals: list2,
        person_room_refusals_forgotten: value.person_room_refusals_forgotten(),
        person_answer_retention: value.person_answer_retention(),
        task_room_refusals: list3,
        task_room_refusals_forgotten: value.task_room_refusals_forgotten(),
        cut: value.cut(),
        connectors: list4,
        connectors_cut: value.connectors_cut(),
    })
}

pub(crate) fn owner_project_resource_role_fields(
    value: &w::OwnerProjectResourceRole,
    _limits: &Limits,
) -> c::OwnerProjectResourceRole {
    match value {
        w::OwnerProjectResourceRole::Owned => c::OwnerProjectResourceRole::Owned,
        w::OwnerProjectResourceRole::Participant => c::OwnerProjectResourceRole::Participant,
        w::OwnerProjectResourceRole::Unavailable => c::OwnerProjectResourceRole::Unavailable,
        w::OwnerProjectResourceRole::Context => c::OwnerProjectResourceRole::Context,
    }
}

pub(crate) fn owner_project_resource_fields(
    value: &w::OwnerProjectResource,
    limits: &Limits,
) -> c::OwnerProjectResource {
    c::OwnerProjectResource {
        part: crate::translate::facts::owner_part(value.part(), limits),
        role: owner_project_resource_role_fields(value.role(), limits),
    }
}
