//! Named limits and strict profile selection (configuration.md, 6).
use crate::{Limits, Path, Problem, Problems, Profile, Segment, What, profile, read};
use skein_json::{Document, document::Kind};
use skein_lib::{Duration, bytes};

/// Replace one whole scalar limit, named by a row of `names::NAMES`.
pub fn apply(limits: &mut Limits, name: &[u8], value: u64) -> Result<(), What> {
    let row = crate::names::find(name).ok_or(What::Override)?;
    field(limits, row.name.as_bytes(), Some(value))?;
    Ok(())
}

/// A named limit's value in `limits`, a duration's in milliseconds; none
/// for a name with no row.
#[must_use]
pub fn value(limits: &Limits, name: &[u8]) -> Option<u64> {
    let row = crate::names::find(name)?;
    let mut copy = *limits;
    field(&mut copy, row.name.as_bytes(), None).ok()
}

fn field(limits: &mut Limits, name: &[u8], to: Option<u64>) -> Result<u64, What> {
    let Limits { core, hub, link, token, driver, .. } = limits;
    if name == b"accounts.token" {
        let value = count(token, to)?;
        if to.is_some() {
            driver.client.document.token_bytes = *token;
        }
        return Ok(value);
    }
    if name == b"accounts.sign_in_deadline_ms" {
        return millis(&mut driver.client.sign_in_time, to);
    }
    let dot = bytes::find(name, b".").ok_or(What::Override)?;
    let owner = name.get(..dot).ok_or(What::Override)?;
    let field = name.get(dot.checked_add(1).ok_or(What::Override)?..).ok_or(What::Override)?;
    match owner {
        b"tasks" => field_tasks(&mut core.tasks, field, to),
        b"people" => field_people(&mut core.people, field, to),
        b"fleet" => field_fleet(&mut core.fleet, field, to),
        b"authority" => {
            let value = field_authority(&mut core.authority, field, to)?;
            if to.is_some() {
                core.documents.roles = core.authority.roles;
            }
            Ok(value)
        }
        b"brief" => field_brief(&mut core.brief, field, to),
        b"accounts" => {
            let value = field_accounts(&mut core.accounts, field, to)?;
            if to.is_some() {
                driver.listeners = core.accounts.sign_ins.checked_mul(2).ok_or(What::Override)?;
                driver.accounts = profile::driver_places(&core.accounts).ok_or(What::Override)?;
            }
            Ok(value)
        }
        b"views" => field_views(&mut core.views, field, to),
        b"notes" => field_notes(&mut core.notes, field, to),
        b"hub" => field_hub(hub, field, to),
        b"documents" => field_documents(&mut core.documents, field, to),
        b"core" => field_core(core, field, to),
        b"link" => field_link(link, field, to),
        b"clients" => match field {
            b"provider_codes" => positive_count(&mut limits.clients.provider_codes, to),
            b"provider_tokens" => positive_count(&mut limits.clients.provider_tokens, to),
            _ => Err(What::Override),
        },
        _ => Err(What::Override),
    }
}

fn count(field: &mut u32, to: Option<u64>) -> Result<u64, What> {
    if let Some(value) = to {
        *field = u32::try_from(value).or(Err(What::Override))?;
    }
    Ok(u64::from(*field))
}

fn positive_count(field: &mut u32, to: Option<u64>) -> Result<u64, What> {
    if to == Some(0) {
        return Err(What::Override);
    }
    count(field, to)
}

fn wide(field: &mut u64, to: Option<u64>) -> u64 {
    if let Some(value) = to {
        *field = value;
    }
    *field
}

fn millis(field: &mut Duration, to: Option<u64>) -> Result<u64, What> {
    if let Some(value) = to {
        if value.checked_mul(1_000_000).is_none() {
            return Err(What::Override);
        }
        *field = Duration::from_millis(value);
    }
    Ok(field.as_nanos() / 1_000_000)
}

#[expect(clippy::too_many_lines, reason = "one exhaustive owner match names every task limit and retry member")]
fn field_tasks(limits: &mut jig_domain_tasks::Limits, name: &[u8], to: Option<u64>) -> Result<u64, What> {
    let jig_domain_tasks::Limits {
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
        b"tasks" => count(tasks, to),
        b"funders" => count(funders, to),
        b"project_tasks" => count(project_tasks, to),
        b"project_goals" => count(project_goals, to),
        b"tree_tasks" => count(tree_tasks, to),
        b"depth" => count(depth, to),
        b"delegates" => count(delegates, to),
        b"references" => count(references, to),
        b"subscriptions" => count(subscriptions, to),
        b"batch" => count(batch, to),
        b"dependencies" => count(dependencies, to),
        b"circle" => count(circle, to),
        b"holdings" => count(holdings, to),
        b"hold_kinds" => count(hold_kinds, to),
        b"resource_reports" => count(resource_reports, to),
        b"pools" => count(pools, to),
        b"hold_segments" => count(hold_segments, to),
        b"hold_bytes" => count(hold_bytes, to),
        b"hold_waiters" => count(hold_waiters, to),
        b"hold_wait_ms" => millis(hold_wait, to),
        b"grace_ms" => millis(grace, to),
        b"inputs" => count(inputs, to),
        b"spec_bytes" => count(spec_bytes, to),
        b"parameters" => count(parameters, to),
        b"result_bytes" => count(result_bytes, to),
        b"run_inbox" => count(run_inbox, to),
        b"inbox_messages" => count(inbox_messages, to),
        b"inbox_bytes" => count(inbox_bytes, to),
        b"message_bytes" => count(message_bytes, to),
        b"person_stall_ms" => millis(person_stall, to),
        b"proposal_stall_ms" => millis(proposal_stall, to),
        b"escalation_stall_ms" => millis(escalation_stall, to),
        b"saved_resources" => count(saved_resources, to),
        b"contract_choices" => count(contract_choices, to),
        b"charters" => count(charters, to),
        b"authority_grants" => count(authority_grants, to),
        b"authority_segments" => count(authority_segments, to),
        b"authority_bytes" => count(authority_bytes, to),
        b"executor_kinds" => count(executor_kinds, to),
        b"facts" => count(facts, to),
        b"room_refusals" => count(room_refusals, to),
        b"retries.transient.retries" => count(transient_retries, to),
        b"retries.transient.base_ms" => millis(transient_base, to),
        b"retries.transient.max_ms" => millis(transient_max, to),
        b"retries.permanent.retries" => count(permanent_retries, to),
        b"retries.permanent.base_ms" => millis(permanent_base, to),
        b"retries.permanent.max_ms" => millis(permanent_max, to),
        b"retries.run.retries" => count(run_retries, to),
        b"retries.run.base_ms" => millis(run_base, to),
        b"retries.run.max_ms" => millis(run_max, to),
        b"retries.agent.retries" => count(agent_retries, to),
        b"retries.agent.base_ms" => millis(agent_base, to),
        b"retries.agent.max_ms" => millis(agent_max, to),
        b"retries.lost.retries" => count(lost_retries, to),
        b"retries.lost.base_ms" => millis(lost_base, to),
        b"retries.lost.max_ms" => millis(lost_max, to),
        b"retries.invalid.retries" => count(invalid_retries, to),
        b"retries.invalid.base_ms" => millis(invalid_base, to),
        b"retries.invalid.max_ms" => millis(invalid_max, to),
        _ => Err(What::Override),
    }
}

fn field_people(limits: &mut jig_domain_people::Limits, name: &[u8], to: Option<u64>) -> Result<u64, What> {
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
        b"program_hosts" => count(program_hosts, to),
        b"program_host_name_bytes" => count(program_host_name_bytes, to),
        b"operations" => count(operations, to),
        b"batch" => count(batch, to),
        b"people" => count(people, to),
        b"inbox_entries" => count(inbox_entries, to),
        b"inbox_tasks" => count(inbox_tasks, to),
        b"inbox_recipients" => count(inbox_recipients, to),
        b"sign_ins" => count(sign_ins, to),
        b"projects" => count(projects, to),
        b"holdings" => count(holdings, to),
        b"initial_owners" => count(initial_owners, to),
        b"requests" => count(requests, to),
        b"room_refusals" => count(room_refusals, to),
        b"pending" => count(pending, to),
        b"waiters" => count(waiters, to),
        b"identity_bytes" => count(identity_bytes, to),
        b"words" => count(words, to),
        b"amendment_bytes" => count(amendment_bytes, to),
        b"sign_in_lifetime_ms" => millis(sign_in_lifetime, to),
        b"request_retention_ms" => millis(request_retention, to),
        b"facts" => count(facts, to),
        _ => Err(What::Override),
    }
}

fn field_fleet(limits: &mut jig_domain_fleet::Limits, name: &[u8], to: Option<u64>) -> Result<u64, What> {
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
        b"configured_workers" => count(configured_workers, to),
        b"worker_name_bytes" => count(worker_name_bytes, to),
        b"program_sources" => count(program_sources, to),
        b"source_detail_bytes" => count(source_detail_bytes, to),
        b"workers" => count(workers, to),
        b"needs_bytes" => Ok(wide(needs_bytes, to)),
        b"engine_slots" => count(engine_slots, to),
        b"slots" => count(slots, to),
        b"workstreams" => count(workstreams, to),
        b"attempts" => count(attempts, to),
        b"grants" => count(grants, to),
        b"calls" => count(calls, to),
        b"call_name_bytes" => Ok(wide(call_name_bytes, to)),
        b"progress_bytes" => Ok(wide(progress_bytes, to)),
        b"turns" => count(turns, to),
        b"grace_ms" => millis(grace, to),
        b"facts" => count(facts, to),
        b"history" => count(history, to),
        _ => Err(What::Override),
    }
}

fn field_authority(limits: &mut jig_domain_authority::Limits, name: &[u8], to: Option<u64>) -> Result<u64, What> {
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
        b"projects" => count(projects, to),
        b"roles" => count(roles, to),
        b"requirements" => count(requirements, to),
        b"facts" => count(facts, to),
        b"grants" => count(grants, to),
        b"executors" => count(executors, to),
        b"segments" => count(segments, to),
        b"segment_bytes" => count(segment_bytes, to),
        b"implications" => count(implications, to),
        b"batch" => count(batch, to),
        b"accounts" => count(accounts, to),
        b"writes" => count(writes, to),
        _ => Err(What::Override),
    }
}

fn field_brief(limits: &mut jig_domain_brief::Limits, name: &[u8], to: Option<u64>) -> Result<u64, What> {
    let jig_domain_brief::Limits { briefs, sections, read_bytes, brief_bytes } = limits;
    match name {
        b"briefs" => count(briefs, to),
        b"sections" => count(sections, to),
        b"read_bytes" => count(read_bytes, to),
        b"brief_bytes" => count(brief_bytes, to),
        _ => Err(What::Override),
    }
}

fn field_accounts(limits: &mut jig_domain_accounts::Limits, name: &[u8], to: Option<u64>) -> Result<u64, What> {
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
        b"program_hosts" => count(program_hosts, to),
        b"accounts" => count(accounts, to),
        b"holder_accounts" => count(holder_accounts, to),
        b"program_names" => count(program_names, to),
        b"sources" => count(sources, to),
        b"sign_ins" => count(sign_ins, to),
        b"credentials" => count(credentials, to),
        b"backoff_base_ms" => millis(backoff_base, to),
        b"backoff_max_ms" => millis(backoff_max, to),
        b"retry_ms" => millis(retry, to),
        b"move_deadline_ms" => millis(move_deadline, to),
        b"spent_attention_ms" => millis(spent_attention, to),
        b"facts" => count(facts, to),
        _ => Err(What::Override),
    }
}

fn field_views(limits: &mut jig_domain_views::Limits, name: &[u8], to: Option<u64>) -> Result<u64, What> {
    let jig_domain_views::Limits { runs, watchers, watchers_per_party, subjects_per_change, text_bytes, facts } =
        limits;
    match name {
        b"runs" => count(runs, to),
        b"watchers" => count(watchers, to),
        b"watchers_per_party" => count(watchers_per_party, to),
        b"subjects_per_change" => count(subjects_per_change, to),
        b"text_bytes" => count(text_bytes, to),
        b"facts" => count(facts, to),
        _ => Err(What::Override),
    }
}

fn field_notes(limits: &mut jig_domain_notes::Limits, name: &[u8], to: Option<u64>) -> Result<u64, What> {
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
        b"scopes" => count(scopes, to),
        b"entries_per_scope" => count(entries_per_scope, to),
        b"pattern_bytes" => count(pattern_bytes, to),
        b"description_bytes" => count(description_bytes, to),
        b"body_bytes" => count(body_bytes, to),
        b"references" => count(references, to),
        b"load_rows" => count(load_rows, to),
        b"lines" => count(lines, to),
        b"recalled" => count(recalled, to),
        _ => Err(What::Override),
    }
}

fn field_hub(limits: &mut jig_domain_host::Limits, name: &[u8], to: Option<u64>) -> Result<u64, What> {
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
        b"program_sources" => count(program_sources, to),
        b"slots" => count(slots, to),
        b"accounts" => count(accounts, to),
        b"charter_bytes" => Ok(wide(charter_bytes, to)),
        b"transcript_bytes" => Ok(wide(transcript_bytes, to)),
        b"delivery_evidence_bytes" => Ok(wide(delivery_evidence_bytes, to)),
        b"turn_bytes" => Ok(wide(turn_bytes, to)),
        b"outcome_bytes" => Ok(wide(outcome_bytes, to)),
        b"detail_bytes" => count(detail_bytes, to),
        b"held" => count(held, to),
        b"inbox" => count(inbox, to),
        b"event_bytes" => Ok(wide(event_bytes, to)),
        b"run_calls" => count(run_calls, to),
        b"facts" => count(facts, to),
        b"told" => count(told, to),
        b"turns" => count(turns, to),
        b"turn_queue_bytes" => Ok(wide(turn_queue_bytes, to)),
        b"piece_bytes" => count(piece_bytes, to),
        _ => Err(What::Override),
    }
}

fn field_documents(
    limits: &mut jig_domain_core::documents::DocumentLimits,
    name: &[u8],
    to: Option<u64>,
) -> Result<u64, What> {
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
        b"rows" => count(rows, to),
        b"text_bytes" => count(text_bytes, to),
        b"chips" => count(chips, to),
        b"lines" => count(lines, to),
        b"versions.minimum" => count(minimum, to),
        b"versions.maximum" => count(maximum, to),
        _ => Err(What::Override),
    }
}

fn field_core(limits: &mut jig_domain_core::Limits, name: &[u8], to: Option<u64>) -> Result<u64, What> {
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
        b"settings.proposals" => positive_count(&mut settings.proposals, to),
        b"settings.revisions" => {
            let value = positive_count(&mut settings.revisions, to)?;
            if to.is_some() {
                tasks.revisions = settings.revisions;
            }
            Ok(value)
        }
        b"settings.versions" => {
            let value = positive_count(&mut settings.versions, to)?;
            if to.is_some() {
                tasks.versions = settings.versions;
                fleet.versions = settings.versions;
            }
            Ok(value)
        }
        b"settings.history" => positive_count(&mut settings.history, to),
        b"settings.alternatives" => count(&mut settings.alternatives, to),
        b"settings.permissions" => count(&mut settings.permissions, to),
        b"templates" => count(templates, to),
        b"fill_mark" => Ok(wide(fill_mark, to)),
        b"connectors" => count(connectors, to),
        b"resume_bytes" => count(resume_bytes, to),
        b"run_bytes" => count(run_bytes, to),
        b"policy_bytes" => Ok(wide(policy_bytes, to)),
        b"escalation_reason_bytes" => count(escalation_reason_bytes, to),
        b"load_slots" => count(load_slots, to),
        b"load_bytes" => Ok(wide(load_bytes, to)),
        b"call_records" => count(call_records, to),
        b"call_answer_bytes" => count(call_answer_bytes, to),
        b"brief_parts" => count(brief_parts, to),
        b"brief_core_budgets.tail" => count(tail, to),
        b"brief_core_budgets.task" => count(task, to),
        b"brief_core_budgets.dependencies" => count(dependencies, to),
        b"brief_core_budgets.attempts" => count(attempts, to),
        b"brief_core_budgets.plan" => count(plan, to),
        b"brief_core_budgets.notes" => count(notes, to),
        _ => Err(What::Override),
    }
}

fn field_link(limits: &mut crate::Link, field: &[u8], to: Option<u64>) -> Result<u64, What> {
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
        b"chunk" => count(chunk, to),
        b"name" => count(name, to),
        b"secret" => count(secret, to),
        b"names" => count(names, to),
        b"opening" => count(opening, to),
        b"handshake_ms" => millis(handshake, to),
        b"hello_ms" => millis(hello, to),
        b"ping_ms" => millis(ping, to),
        b"silence_ms" => millis(silence, to),
        b"stall_ms" => millis(stall, to),
        b"backoff_ms" => millis(backoff, to),
        b"ceiling_ms" => millis(ceiling, to),
        b"proved_ms" => millis(proved, to),
        b"busy_ms" => millis(busy, to),
        b"facts" => count(facts, to),
        _ => Err(What::Override),
    }
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
        Some(b"clients") => jig_protocol_engine_clients::worst_case(&limits.clients).is_some(),
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
