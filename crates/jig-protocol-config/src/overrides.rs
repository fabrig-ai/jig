//! Named limits and strict profile selection (configuration.md, 6).
use crate::{Limits, Path, Problem, Problems, Profile, Segment, What, profile, read};
use skein_json::{Document, document::Kind};
use skein_lib::{Duration, bytes};

/// Replace one whole scalar limit. The composed reader checks the resulting profile.
pub fn apply(limits: &mut Limits, name: &[u8], value: u64) -> Result<(), What> {
    let Limits { core, hub, link, token, driver, .. } = limits;
    if name == b"accounts.token" {
        *token = count(value)?;
        driver.client.document.token_bytes = *token;
        return Ok(());
    }
    if name == b"accounts.sign_in_deadline_ms" {
        driver.client.sign_in_time = millis(value)?;
        return Ok(());
    }
    let dot = bytes::find(name, b".").ok_or(What::Override)?;
    let owner = name.get(..dot).ok_or(What::Override)?;
    let field = name.get(dot.checked_add(1).ok_or(What::Override)?..).ok_or(What::Override)?;
    match owner {
        b"store" => set_store(&mut limits.store.store, field, value),
        b"secrets" => set_store(&mut limits.store.secrets, field, value),
        b"tasks" => set_tasks(&mut core.tasks, field, value),
        b"people" => set_people(&mut core.people, field, value),
        b"fleet" => set_fleet(&mut core.fleet, field, value),
        b"authority" => {
            set_authority(&mut core.authority, field, value)?;
            core.documents.roles = core.authority.roles;
            Ok(())
        }
        b"brief" => set_brief(&mut core.brief, field, value),
        b"accounts" => {
            set_accounts(&mut core.accounts, field, value)?;
            driver.listeners = core.accounts.sign_ins.checked_mul(2).ok_or(What::Override)?;
            driver.accounts = profile::driver_places(&core.accounts).ok_or(What::Override)?;
            Ok(())
        }
        b"views" => set_views(&mut core.views, field, value),
        b"notes" => set_notes(&mut core.notes, field, value),
        b"hub" => set_hub(hub, field, value),
        b"documents" => set_documents(&mut core.documents, field, value),
        b"core" => set_core(core, field, value),
        b"link" => set_link(link, field, value),
        b"clients" => {
            let count = count(value)?;
            if count == 0 {
                return Err(What::Override);
            }
            match field {
                b"provider_codes" => limits.clients.provider_codes = count,
                b"provider_tokens" => limits.clients.provider_tokens = count,
                _ => return Err(What::Override),
            }
            Ok(())
        }
        _ => Err(What::Override),
    }
}
fn set_store(limits: &mut skein_kv::Limits, name: &[u8], value: u64) -> Result<(), What> {
    if name != b"budget" {
        return Err(What::Override);
    }
    *limits = profile::store_budget(*limits, value).ok_or(What::Override)?;
    Ok(())
}

fn count(value: u64) -> Result<u32, What> {
    u32::try_from(value).or(Err(What::Override))
}
fn millis(value: u64) -> Result<Duration, What> {
    if value.checked_mul(1_000_000).is_none() {
        return Err(What::Override);
    }
    Ok(Duration::from_millis(value))
}

#[expect(clippy::too_many_lines, reason = "one exhaustive owner match names every task limit and retry member")]
fn set_tasks(limits: &mut jig_domain_tasks::Limits, name: &[u8], value: u64) -> Result<(), What> {
    let jig_domain_tasks::Limits {
        placed_holds,
        project_placed_holds,
        party_placed_holds,
        exemptions,
        tasks,
        funders,
        project_tasks,
        project_goals,
        tree_tasks,
        depth,
        delegates,
        references,
        subscriptions,
        batch,
        dependencies,
        circle,
        holdings,
        hold_kinds,
        resource_reports,
        pools,
        hold_segments,
        hold_bytes,
        hold_waiters,
        hold_wait,
        grace,
        inputs,
        spec_bytes,
        parameters,
        result_bytes,
        run_inbox,
        inbox_messages,
        inbox_bytes,
        message_bytes,
        person_stall,
        proposal_stall,
        escalation_stall,
        saved_resources,
        contract_choices,
        charters,
        versions: _,
        revisions: _,
        authority_grants,
        authority_segments,
        authority_bytes,
        executor_kinds,
        retries,
        facts,
        room_refusals,
    } = limits;
    let jig_domain_tasks::Retries { transient, permanent, run, agent, lost, invalid } = retries;
    let jig_domain_tasks::Retry { retries: transient_retries, base: transient_base, max: transient_max } = transient;
    let jig_domain_tasks::Retry { retries: permanent_retries, base: permanent_base, max: permanent_max } = permanent;
    let jig_domain_tasks::Retry { retries: run_retries, base: run_base, max: run_max } = run;
    let jig_domain_tasks::Retry { retries: agent_retries, base: agent_base, max: agent_max } = agent;
    let jig_domain_tasks::Retry { retries: lost_retries, base: lost_base, max: lost_max } = lost;
    let jig_domain_tasks::Retry { retries: invalid_retries, base: invalid_base, max: invalid_max } = invalid;
    match name {
        b"placed_holds" => *placed_holds = count(value)?,
        b"project_placed_holds" => *project_placed_holds = count(value)?,
        b"party_placed_holds" => *party_placed_holds = count(value)?,
        b"exemptions" => *exemptions = count(value)?,
        b"tasks" => *tasks = count(value)?,
        b"funders" => *funders = count(value)?,
        b"project_tasks" => *project_tasks = count(value)?,
        b"project_goals" => *project_goals = count(value)?,
        b"tree_tasks" => *tree_tasks = count(value)?,
        b"depth" => *depth = count(value)?,
        b"delegates" => *delegates = count(value)?,
        b"references" => *references = count(value)?,
        b"subscriptions" => *subscriptions = count(value)?,
        b"batch" => *batch = count(value)?,
        b"dependencies" => *dependencies = count(value)?,
        b"circle" => *circle = count(value)?,
        b"holdings" => *holdings = count(value)?,
        b"hold_kinds" => *hold_kinds = count(value)?,
        b"resource_reports" => *resource_reports = count(value)?,
        b"pools" => *pools = count(value)?,
        b"hold_segments" => *hold_segments = count(value)?,
        b"hold_bytes" => *hold_bytes = count(value)?,
        b"hold_waiters" => *hold_waiters = count(value)?,
        b"hold_wait_ms" => *hold_wait = millis(value)?,
        b"grace_ms" => *grace = millis(value)?,
        b"inputs" => *inputs = count(value)?,
        b"spec_bytes" => *spec_bytes = count(value)?,
        b"parameters" => *parameters = count(value)?,
        b"result_bytes" => *result_bytes = count(value)?,
        b"run_inbox" => *run_inbox = count(value)?,
        b"inbox_messages" => *inbox_messages = count(value)?,
        b"inbox_bytes" => *inbox_bytes = count(value)?,
        b"message_bytes" => *message_bytes = count(value)?,
        b"person_stall_ms" => *person_stall = millis(value)?,
        b"proposal_stall_ms" => *proposal_stall = millis(value)?,
        b"escalation_stall_ms" => *escalation_stall = millis(value)?,
        b"saved_resources" => *saved_resources = count(value)?,
        b"contract_choices" => *contract_choices = count(value)?,
        b"charters" => *charters = count(value)?,
        b"authority_grants" => *authority_grants = count(value)?,
        b"authority_segments" => *authority_segments = count(value)?,
        b"authority_bytes" => *authority_bytes = count(value)?,
        b"executor_kinds" => *executor_kinds = count(value)?,
        b"facts" => *facts = count(value)?,
        b"room_refusals" => *room_refusals = count(value)?,
        b"retries.transient.retries" => *transient_retries = count(value)?,
        b"retries.transient.base_ms" => *transient_base = millis(value)?,
        b"retries.transient.max_ms" => *transient_max = millis(value)?,
        b"retries.permanent.retries" => *permanent_retries = count(value)?,
        b"retries.permanent.base_ms" => *permanent_base = millis(value)?,
        b"retries.permanent.max_ms" => *permanent_max = millis(value)?,
        b"retries.run.retries" => *run_retries = count(value)?,
        b"retries.run.base_ms" => *run_base = millis(value)?,
        b"retries.run.max_ms" => *run_max = millis(value)?,
        b"retries.agent.retries" => *agent_retries = count(value)?,
        b"retries.agent.base_ms" => *agent_base = millis(value)?,
        b"retries.agent.max_ms" => *agent_max = millis(value)?,
        b"retries.lost.retries" => *lost_retries = count(value)?,
        b"retries.lost.base_ms" => *lost_base = millis(value)?,
        b"retries.lost.max_ms" => *lost_max = millis(value)?,
        b"retries.invalid.retries" => *invalid_retries = count(value)?,
        b"retries.invalid.base_ms" => *invalid_base = millis(value)?,
        b"retries.invalid.max_ms" => *invalid_max = millis(value)?,
        _ => return Err(What::Override),
    }
    Ok(())
}

fn set_people(limits: &mut jig_domain_people::Limits, name: &[u8], value: u64) -> Result<(), What> {
    let jig_domain_people::Limits {
        program_hosts,
        program_host_name_bytes,
        act_grants: _,
        operations,
        batch,
        people,
        inbox_entries,
        inbox_tasks,
        inbox_recipients,
        sign_ins,
        projects,
        holdings,
        initial_owners,
        requests,
        room_refusals,
        pending,
        waiters,
        identity_bytes,
        words,
        amendment_bytes,
        sign_in_lifetime,
        request_retention,
        facts,
    } = limits;
    match name {
        b"program_hosts" => *program_hosts = count(value)?,
        b"program_host_name_bytes" => *program_host_name_bytes = count(value)?,
        b"operations" => *operations = count(value)?,
        b"batch" => *batch = count(value)?,
        b"people" => *people = count(value)?,
        b"inbox_entries" => *inbox_entries = count(value)?,
        b"inbox_tasks" => *inbox_tasks = count(value)?,
        b"inbox_recipients" => *inbox_recipients = count(value)?,
        b"sign_ins" => *sign_ins = count(value)?,
        b"projects" => *projects = count(value)?,
        b"holdings" => *holdings = count(value)?,
        b"initial_owners" => *initial_owners = count(value)?,
        b"requests" => *requests = count(value)?,
        b"room_refusals" => *room_refusals = count(value)?,
        b"pending" => *pending = count(value)?,
        b"waiters" => *waiters = count(value)?,
        b"identity_bytes" => *identity_bytes = count(value)?,
        b"words" => *words = count(value)?,
        b"amendment_bytes" => *amendment_bytes = count(value)?,
        b"sign_in_lifetime_ms" => *sign_in_lifetime = millis(value)?,
        b"request_retention_ms" => *request_retention = millis(value)?,
        b"facts" => *facts = count(value)?,
        _ => return Err(What::Override),
    }
    Ok(())
}

fn set_fleet(limits: &mut jig_domain_fleet::Limits, name: &[u8], value: u64) -> Result<(), What> {
    let jig_domain_fleet::Limits {
        configured_workers,
        worker_name_bytes,
        program_sources,
        source_detail_bytes,
        workers,
        needs_bytes,
        versions: _,
        engine_slots,
        slots,
        workstreams,
        attempts,
        grants,
        calls,
        call_name_bytes,
        progress_bytes,
        turns,
        grace,
        facts,
        history,
    } = limits;
    match name {
        b"configured_workers" => *configured_workers = count(value)?,
        b"worker_name_bytes" => *worker_name_bytes = count(value)?,
        b"program_sources" => *program_sources = count(value)?,
        b"source_detail_bytes" => *source_detail_bytes = count(value)?,
        b"workers" => *workers = count(value)?,
        b"needs_bytes" => *needs_bytes = value,
        b"engine_slots" => *engine_slots = count(value)?,
        b"slots" => *slots = count(value)?,
        b"workstreams" => *workstreams = count(value)?,
        b"attempts" => *attempts = count(value)?,
        b"grants" => *grants = count(value)?,
        b"calls" => *calls = count(value)?,
        b"call_name_bytes" => *call_name_bytes = value,
        b"progress_bytes" => *progress_bytes = value,
        b"turns" => *turns = count(value)?,
        b"grace_ms" => *grace = millis(value)?,
        b"facts" => *facts = count(value)?,
        b"history" => *history = count(value)?,
        _ => return Err(What::Override),
    }
    Ok(())
}

fn set_authority(limits: &mut jig_domain_authority::Limits, name: &[u8], value: u64) -> Result<(), What> {
    let jig_domain_authority::Limits {
        projects,
        roles,
        requirements,
        facts,
        grants,
        executors,
        segments,
        segment_bytes,
        implications,
        batch,
        accounts,
        writes,
    } = limits;
    match name {
        b"projects" => *projects = count(value)?,
        b"roles" => *roles = count(value)?,
        b"requirements" => *requirements = count(value)?,
        b"facts" => *facts = count(value)?,
        b"grants" => *grants = count(value)?,
        b"executors" => *executors = count(value)?,
        b"segments" => *segments = count(value)?,
        b"segment_bytes" => *segment_bytes = count(value)?,
        b"implications" => *implications = count(value)?,
        b"batch" => *batch = count(value)?,
        b"accounts" => *accounts = count(value)?,
        b"writes" => *writes = count(value)?,
        _ => return Err(What::Override),
    }
    Ok(())
}

fn set_brief(limits: &mut jig_domain_brief::Limits, name: &[u8], value: u64) -> Result<(), What> {
    let jig_domain_brief::Limits { briefs, sections, read_bytes, brief_bytes } = limits;
    match name {
        b"briefs" => *briefs = count(value)?,
        b"sections" => *sections = count(value)?,
        b"read_bytes" => *read_bytes = count(value)?,
        b"brief_bytes" => *brief_bytes = count(value)?,
        _ => return Err(What::Override),
    }
    Ok(())
}

fn set_accounts(limits: &mut jig_domain_accounts::Limits, name: &[u8], value: u64) -> Result<(), What> {
    let jig_domain_accounts::Limits {
        program_hosts,
        accounts,
        holder_accounts,
        program_names,
        sources,
        sign_ins,
        credentials,
        backoff_base,
        backoff_max,
        retry,
        move_deadline,
        spent_attention,
        facts,
    } = limits;
    match name {
        b"program_hosts" => *program_hosts = count(value)?,
        b"accounts" => *accounts = count(value)?,
        b"holder_accounts" => *holder_accounts = count(value)?,
        b"program_names" => *program_names = count(value)?,
        b"sources" => *sources = count(value)?,
        b"sign_ins" => *sign_ins = count(value)?,
        b"credentials" => *credentials = count(value)?,
        b"backoff_base_ms" => *backoff_base = millis(value)?,
        b"backoff_max_ms" => *backoff_max = millis(value)?,
        b"retry_ms" => *retry = millis(value)?,
        b"move_deadline_ms" => *move_deadline = millis(value)?,
        b"spent_attention_ms" => *spent_attention = millis(value)?,
        b"facts" => *facts = count(value)?,
        _ => return Err(What::Override),
    }
    Ok(())
}

fn set_views(limits: &mut jig_domain_views::Limits, name: &[u8], value: u64) -> Result<(), What> {
    let jig_domain_views::Limits { runs, watchers, watchers_per_party, subjects_per_change, text_bytes, facts } =
        limits;
    match name {
        b"runs" => *runs = count(value)?,
        b"watchers" => *watchers = count(value)?,
        b"watchers_per_party" => *watchers_per_party = count(value)?,
        b"subjects_per_change" => *subjects_per_change = count(value)?,
        b"text_bytes" => *text_bytes = count(value)?,
        b"facts" => *facts = count(value)?,
        _ => return Err(What::Override),
    }
    Ok(())
}

fn set_notes(limits: &mut jig_domain_notes::Limits, name: &[u8], value: u64) -> Result<(), What> {
    let jig_domain_notes::Limits {
        scopes,
        entries_per_scope,
        pattern_bytes,
        description_bytes,
        body_bytes,
        references,
        load_rows,
        lines,
        recalled,
    } = limits;
    match name {
        b"scopes" => *scopes = count(value)?,
        b"entries_per_scope" => *entries_per_scope = count(value)?,
        b"pattern_bytes" => *pattern_bytes = count(value)?,
        b"description_bytes" => *description_bytes = count(value)?,
        b"body_bytes" => *body_bytes = count(value)?,
        b"references" => *references = count(value)?,
        b"load_rows" => *load_rows = count(value)?,
        b"lines" => *lines = count(value)?,
        b"recalled" => *recalled = count(value)?,
        _ => return Err(What::Override),
    }
    Ok(())
}

fn set_hub(limits: &mut jig_domain_host::Limits, name: &[u8], value: u64) -> Result<(), What> {
    let jig_domain_host::Limits {
        program_sources,
        source_interval: _,
        source_timeout: _,
        slots,
        accounts,
        charter_bytes,
        transcript_bytes,
        delivery_evidence_bytes,
        turn_bytes,
        outcome_bytes,
        detail_bytes,
        held,
        inbox,
        event_bytes,
        run_calls,
        facts,
        told,
        turns,
        turn_queue_bytes,
        piece_bytes,
    } = limits;
    match name {
        b"program_sources" => *program_sources = count(value)?,
        b"slots" => *slots = count(value)?,
        b"accounts" => *accounts = count(value)?,
        b"charter_bytes" => *charter_bytes = value,
        b"transcript_bytes" => *transcript_bytes = value,
        b"delivery_evidence_bytes" => *delivery_evidence_bytes = value,
        b"turn_bytes" => *turn_bytes = value,
        b"outcome_bytes" => *outcome_bytes = value,
        b"detail_bytes" => *detail_bytes = count(value)?,
        b"held" => *held = count(value)?,
        b"inbox" => *inbox = count(value)?,
        b"event_bytes" => *event_bytes = value,
        b"run_calls" => *run_calls = count(value)?,
        b"facts" => *facts = count(value)?,
        b"told" => *told = count(value)?,
        b"turns" => *turns = count(value)?,
        b"turn_queue_bytes" => *turn_queue_bytes = value,
        b"piece_bytes" => *piece_bytes = count(value)?,
        _ => return Err(What::Override),
    }
    Ok(())
}

fn set_documents(limits: &mut jig_domain_core::documents::DocumentLimits, name: &[u8], value: u64) -> Result<(), What> {
    let jig_domain_core::documents::DocumentLimits {
        roles: _,
        rows,
        text_bytes,
        chips,
        lines,
        versions,
        segments: _,
        grants: _,
        executors: _,
        implications: _,
    } = limits;
    let jig_domain_core::documents::Versions { minimum, maximum } = versions;
    match name {
        b"rows" => *rows = count(value)?,
        b"text_bytes" => *text_bytes = count(value)?,
        b"chips" => *chips = count(value)?,
        b"lines" => *lines = count(value)?,
        b"versions.minimum" => *minimum = count(value)?,
        b"versions.maximum" => *maximum = count(value)?,
        _ => return Err(What::Override),
    }
    Ok(())
}

fn set_core(limits: &mut jig_domain_core::Limits, name: &[u8], value: u64) -> Result<(), What> {
    let jig_domain_core::Limits {
        settings,
        templates,
        fill_mark,
        connectors,
        resume_bytes,
        run_bytes,
        policy_bytes,
        escalation_reason_bytes,
        load_slots,
        load_bytes,
        call_records,
        call_answer_bytes,
        tasks,
        authority: _,
        people: _,
        fleet,
        brief: _,
        brief_parts,
        brief_core_budgets,
        accounts: _,
        notes: _,
        documents: _,
        views: _,
    } = limits;
    let jig_domain_core::CoreBriefBudgets { tail, task, dependencies, attempts, plan, notes } = brief_core_budgets;
    match name {
        b"settings.proposals" => {
            if value == 0 {
                return Err(What::Override);
            }
            settings.proposals = count(value)?;
        }
        b"settings.revisions" | b"settings.versions" => {
            if value == 0 {
                return Err(What::Override);
            }
            if name == b"settings.revisions" {
                settings.revisions = count(value)?;
                tasks.revisions = count(value)?;
            } else {
                settings.versions = count(value)?;
                tasks.versions = count(value)?;
                fleet.versions = count(value)?;
            }
        }
        b"settings.history" => {
            if value == 0 {
                return Err(What::Override);
            }
            settings.history = count(value)?;
        }
        b"settings.alternatives" => settings.alternatives = count(value)?,
        b"settings.permissions" => settings.permissions = count(value)?,
        b"templates" => *templates = count(value)?,
        b"fill_mark" => *fill_mark = value,
        b"connectors" => *connectors = count(value)?,
        b"resume_bytes" => *resume_bytes = count(value)?,
        b"run_bytes" => *run_bytes = count(value)?,
        b"policy_bytes" => *policy_bytes = value,
        b"escalation_reason_bytes" => *escalation_reason_bytes = count(value)?,
        b"load_slots" => *load_slots = count(value)?,
        b"load_bytes" => *load_bytes = value,
        b"call_records" => *call_records = count(value)?,
        b"call_answer_bytes" => *call_answer_bytes = count(value)?,
        b"brief_parts" => *brief_parts = count(value)?,
        b"brief_core_budgets.tail" => *tail = count(value)?,
        b"brief_core_budgets.task" => *task = count(value)?,
        b"brief_core_budgets.dependencies" => *dependencies = count(value)?,
        b"brief_core_budgets.attempts" => *attempts = count(value)?,
        b"brief_core_budgets.plan" => *plan = count(value)?,
        b"brief_core_budgets.notes" => *notes = count(value)?,
        _ => return Err(What::Override),
    }
    Ok(())
}

fn set_link(limits: &mut crate::Link, field: &[u8], value: u64) -> Result<(), What> {
    let crate::Link {
        chunk,
        name,
        secret,
        names,
        opening,
        handshake,
        hello,
        ping,
        silence,
        stall,
        backoff,
        ceiling,
        proved,
        busy,
        facts,
    } = limits;
    match field {
        b"chunk" => *chunk = count(value)?,
        b"name" => *name = count(value)?,
        b"secret" => *secret = count(value)?,
        b"names" => *names = count(value)?,
        b"opening" => *opening = count(value)?,
        b"handshake_ms" => *handshake = millis(value)?,
        b"hello_ms" => *hello = millis(value)?,
        b"ping_ms" => *ping = millis(value)?,
        b"silence_ms" => *silence = millis(value)?,
        b"stall_ms" => *stall = millis(value)?,
        b"backoff_ms" => *backoff = millis(value)?,
        b"ceiling_ms" => *ceiling = millis(value)?,
        b"proved_ms" => *proved = millis(value)?,
        b"busy_ms" => *busy = millis(value)?,
        b"facts" => *facts = count(value)?,
        _ => return Err(What::Override),
    }
    Ok(())
}

/// Read the limits member at its path, then check all overrides together.
pub fn parse(document: &Document, at: u32, path: &Path) -> Result<Limits, Problems> {
    let mut problems = Problems::empty();
    let mut object = match read::Object::new(document, at) {
        Ok(object) => object,
        Err(what) => {
            problems.push(path.clone(), what);
            return Err(problems);
        }
    };
    let mut limits = profile(Profile::Small);
    if let Some(at) = object.required(document, b"profile", path, &mut problems) {
        match read::text(document, at, 8) {
            Ok(name) => match name.as_ref() {
                b"small" => {}
                b"standard" => limits = profile(Profile::Standard),
                _ => problems.push(path.member(b"profile"), What::Override),
            },
            Err(what) => problems.push(path.member(b"profile"), what),
        }
    }
    let overrides_at = object.member(document, b"overrides");
    if let Some(at) = overrides_at {
        let path = path.member(b"overrides");
        if let Err(what) = members(document, at, &path, &mut limits, &mut problems) {
            problems.push(path, what);
        }
    }
    limits.core.documents.segments = profile::name_segments(&limits.core.tasks, &limits.core.authority);
    limits.core.documents.grants = profile::authority_grants(&limits.core.tasks, &limits.core.authority);
    limits.core.documents.executors = profile::authority_executors(&limits.core.tasks, &limits.core.authority);
    limits.core.documents.implications = limits.core.authority.implications;
    object.finish(document, path, &mut problems);
    crate::clients::check_settings(&limits, &mut problems);
    if let Err(what) = crate::domain::check_message(&limits.core) {
        problems.push(path.clone(), what);
    }
    if problems.is_empty()
        && let Err(problem) = crate::check_effective_limits(&limits.core)
    {
        let problem = codec_problem(document, overrides_at, path, problem);
        problems.push(problem.path, problem.what);
    }
    if problems.is_empty() { Ok(limits) } else { Err(problems) }
}

// A canonical effective-limit path is translated back to its spelling in the
// override document when that member was present, including a caller's prefix.
fn codec_problem(document: &Document, overrides_at: Option<u32>, path: &Path, problem: Problem) -> Problem {
    if let Some(at) = overrides_at {
        let end = read::after(document, at)
            .expect("override object was checked")
            .checked_sub(1)
            .expect("object has an end token");
        let mut next = at.checked_add(1).expect("checked document position");
        for _ in at..end {
            if next == end {
                break;
            }
            let key = document.token(next).expect("checked override key");
            let name = document.text(key).expect("checked override name");
            if crate::domain::codec_path(name) == problem.path {
                return Problem { path: path.member(b"overrides").member(name), what: problem.what };
            }
            let value = next.checked_add(1).expect("checked override value");
            next = read::after(document, value).expect("override value was checked");
        }
    }
    let mut complete = path.clone();
    for (index, segment) in problem.path.segments.iter().enumerate() {
        if index > 0 {
            complete = complete.child(match segment {
                Segment::Member(name) => Segment::Member(bytes::copy_of(name)),
                Segment::Index(index) => Segment::Index(*index),
            });
        }
    }
    Problem { path: complete, what: problem.what }
}

fn members(
    document: &Document,
    at: u32,
    path: &Path,
    limits: &mut Limits,
    problems: &mut Problems,
) -> Result<(), What> {
    let mut object = read::Object::new(document, at)?;
    let end = read::after(document, at)?.checked_sub(1).ok_or(What::Kind)?;
    let mut next = at.checked_add(1).ok_or(What::Bound)?;
    for _ in at..end {
        if next == end {
            break;
        }
        let key = document.token(next).ok_or(What::Kind)?;
        if key.kind != Kind::Key {
            return Err(What::Kind);
        }
        let name = document.text(key).ok_or(What::Kind)?;
        let value_at = next.checked_add(1).ok_or(What::Bound)?;
        if object.member(document, name) == Some(value_at) {
            let value = read::integer(document, value_at, u64::MAX).or(Err(What::Override));
            let result = match value {
                Ok(value) => apply(limits, name, value),
                Err(what) => Err(what),
            };
            if let Err(what) = result {
                problems.push(path.member(name), what);
            }
        }
        next = read::after(document, value_at)?;
    }
    object.finish(document, path, problems);
    if !problems.is_empty() {
        return Ok(());
    }
    next = at.checked_add(1).ok_or(What::Bound)?;
    let mut fits = Fits { hub: None, core: None };
    for _ in at..end {
        if next == end {
            break;
        }
        let name = document.text(document.token(next).ok_or(What::Kind)?).ok_or(What::Kind)?;
        if !valid(limits, name, &mut fits) {
            problems.push(path.member(name), What::Override);
        }
        next = read::after(document, next.checked_add(1).ok_or(What::Bound)?)?;
    }
    Ok(())
}

/// Each family's worst case over the overridden limits, computed once when first asked.
struct Fits {
    hub: Option<bool>,
    core: Option<bool>,
}

fn valid(limits: &Limits, name: &[u8], fits: &mut Fits) -> bool {
    if (name == b"accounts.sign_ins" || name == b"accounts.sign_in_deadline_ms") && !driver_fits(&limits.driver) {
        return false;
    }
    let Some(dot) = bytes::find(name, b".") else {
        return false;
    };
    match name.get(..dot) {
        Some(b"hub") => {
            if fits.hub.is_none() {
                fits.hub = Some(jig_domain_host::worst_case(&limits.hub).is_some());
            }
            fits.hub.expect("hub fit checked")
        }
        Some(b"link") => true,
        Some(b"store") => store_fits(&limits.store.store),
        Some(b"secrets") => store_fits(&limits.store.secrets),
        Some(
            b"core" | b"tasks" | b"people" | b"fleet" | b"authority" | b"brief" | b"accounts" | b"notes" | b"views"
            | b"documents",
        ) => {
            if fits.core.is_none() {
                fits.core = Some(jig_domain_core::worst_case(&limits.core).is_some());
            }
            fits.core.expect("core fit checked")
        }
        Some(_) | None => false,
    }
}

// A store must admit its largest value and a full one-operation record,
// in addition to the KV component's own usability and memory checks.
fn store_fits(limits: &skein_kv::Limits) -> bool {
    let record = u64::from(limits.key).saturating_add(u64::from(limits.value)).saturating_add(35);
    limits.budget >= u64::from(limits.value)
        && u64::from(limits.commit) >= record
        && skein_kv::worst_case(limits).is_some()
}

/// The accounts' driver listens for each sign-in at once, at least one, on two sockets each beside
/// its exchanges', and a sign-in waits within a deadline above zero (skein's oauth.md, 6.7).
fn driver_fits(driver: &skein_oauth_accounts::Limits) -> bool {
    let Some(listening) = driver.listeners.checked_mul(2) else {
        return false;
    };
    let Some(sockets) = listening.checked_add(driver.exchanges) else {
        return false;
    };
    driver.client.url_bytes <= jig_domain_accounts::ADDRESS
        && driver.listeners > 0
        && sockets <= driver.io.sockets
        && driver.client.sign_in_time > Duration::ZERO
        && skein_oauth_accounts::worst_case(driver).is_some()
}
