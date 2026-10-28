//! Strict configuration values and operator problems (protocol/configuration.md, 1 and 10).
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
extern crate alloc;
use alloc::boxed::Box;
mod acts;
mod charters;
mod core;
mod templates;
pub use core::core;
mod directories;
pub use directories::{Directories, Directory};
mod defaults;
mod domain;
pub use defaults::pass;
mod projects;
pub use domain::{Deployment, Provider, ProviderKind, amendment_bytes_max, check_effective_limits, policy_bytes_max};
pub use projects::{Policy, Project};
mod check;
mod worker;
pub use worker::{Agent, Graces, Worker, stop_bound, worker};
pub mod clients;
mod files;
pub mod hosts;
pub mod llm;
pub use clients::Clients;
pub use files::{File, files, worker_files};
pub use hosts::{Hosts, WorkerName};
pub use llm::{Endpoint, Header, Llm, Transport};
pub mod address;
mod document;
pub mod overrides;
pub mod profile;
pub use profile::{Limits, Link, Profile, profile};
pub mod forms;
mod problem;
pub mod read;
pub use address::Address;
pub use document::{DocumentLimits, document};
pub use forms::{digest, secret};
pub use problem::{CodecBound, PROBLEMS, Path, Problem, Problems, Segment, What};
/// Where an engine's settings defaults come from (configuration.md, 1;
/// domain/engine.md, 17.1 and 17.7).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Defaults {
    /// The document's `charters`, `templates`, `policies` and `rules`.
    Document,
    /// The application's defaults, given by `pass`; the four members are refused.
    Passed,
}
/// The engine's protocol configuration (configuration.md, 3).
#[derive(Debug)]
pub struct Engine {
    /// With `Passed`, the four settings fields are empty until `pass` gives them.
    pub defaults: Defaults,
    pub deployment: Deployment,
    pub owners: Box<[jig_domain_people::InitialOwner]>,
    pub acts: Box<[jig_domain_people::ActGrant]>,
    pub providers: Box<[Provider]>,
    pub rules: jig_domain_authority::Rules,
    pub projects: Box<[Project]>,
    /// Policy numbers awaiting defaults, by seed index; empty once the copy is resolved.
    pub seed_policies: Box<[Option<u32>]>,
    pub policies: Box<[Policy]>,
    pub connectors: Box<[u16]>,
    pub memory: u64,
    pub charters: Box<[jig_domain_core::Charter]>,
    pub templates: Box<[jig_domain_core::Template]>,
    pub llm: Llm,
    pub hosts: Hosts,
    pub clients: Clients,
    pub limits: Limits,
}
/// Parse jig's engine member before startup constructs any component.
pub fn engine(
    document: &skein_json::Document,
    at: u32,
    directories: &Directories,
    defaults: Defaults,
) -> Result<Engine, Problems> {
    let path = Path::root();
    let mut problems = Problems::empty();
    let Some(mut object) = check::object(document, at, &path, &mut problems) else {
        return Err(problems);
    };
    let mut limits = profile(Profile::Small);
    if let Some(at) = object.required(document, b"limits", &path, &mut problems) {
        match overrides::parse(document, at, &path.member(b"limits")) {
            Ok(value) => limits = value,
            Err(found) => {
                for problem in &found.problems {
                    problems.push(problem.path.clone(), problem.what);
                }
                problems.past = problems.past.saturating_add(found.past);
            }
        }
    }
    let mut llm = None;
    if let Some(at) = object.required(document, b"llm", &path, &mut problems) {
        llm = llm::parse(document, at, &path.member(b"llm"), directories, &mut problems);
    }
    let mut hosts = None;
    if let Some(at) = object.required(document, b"hosts", &path, &mut problems) {
        hosts = hosts::parse(document, at, &path.member(b"hosts"), &limits, directories, &mut problems);
    }
    let mut clients = None;
    if let Some(at) = object.required(document, b"clients", &path, &mut problems) {
        clients = clients::parse(document, at, &path.member(b"clients"), directories, &mut problems);
    }
    let offers = defaults::offers(document, &mut object, llm.as_ref(), &limits, directories, defaults, &mut problems);
    let configured = match &llm {
        Some(llm) => &*llm.models,
        None => &[],
    };
    let mut deployment = None;
    if let Some(at) = object.required(document, b"deployment", &path, &mut problems) {
        deployment = domain::deployment(document, at, &path.member(b"deployment"), directories, &mut problems);
    }
    let mut providers = None;
    if let Some(at) = object.required(document, b"providers", &path, &mut problems) {
        providers = domain::providers(document, at, &path.member(b"providers"), &mut problems);
    }
    let mut rules = if defaults == Defaults::Passed { Some(defaults::empty_rules()) } else { None };
    if defaults == Defaults::Document
        && let Some(at) = object.required(document, b"rules", &path, &mut problems)
    {
        let authority = &limits.core.authority;
        rules = projects::rules(document, at, &path.member(b"rules"), authority, configured, &mut problems);
    }
    let mut projects = None;
    if let Some(at) = object.required(document, b"projects", &path, &mut problems) {
        projects = projects::projects(document, at, &path.member(b"projects"), &limits, configured, &mut problems);
    }
    let policies = if defaults == Defaults::Document {
        projects::configured_policies(&mut object, document, &limits, configured, &mut problems)
    } else {
        Some(Box::default())
    };
    let mut connectors = None;
    if let Some(at) = object.required(document, b"connectors", &path, &mut problems) {
        connectors =
            domain::connectors(document, at, &path.member(b"connectors"), limits.core.connectors, &mut problems);
    }
    let acts = acts::parse(&mut object, document, &path, connectors.as_deref().unwrap_or(&[]), &mut problems);
    limits.core.people.act_grants = u32::try_from(acts.len()).expect("document count fits u32");
    let memory = check::integer(&mut object, document, b"memory_bytes", &path, &mut problems);
    let owners = object.required(document, b"owners", &path, &mut problems);
    object.finish(document, &path, &mut problems);
    if !problems.is_empty() {
        return Err(problems);
    }
    let projects = projects.expect("required member checked");
    let mut engine = Engine {
        defaults,
        deployment: deployment.expect("required member checked"),
        providers: providers.expect("required member checked"),
        rules: rules.expect("required member checked"),
        projects: projects.values,
        seed_policies: projects.policies,
        policies: policies.expect("optional policies checked"),
        connectors: connectors.expect("required member checked"),
        owners: Box::default(),
        acts,
        memory: memory.expect("required member checked"),
        charters: offers.charters.expect("required member checked"),
        templates: offers.templates.expect("required member checked"),
        llm: llm.expect("required member checked"),
        hosts: hosts.expect("required member checked"),
        clients: clients.expect("required member checked"),
        limits,
    };
    engine.owners = domain::owners(document, owners.expect("required member checked"), &engine, &mut problems);
    clients::check_provider(&engine, &mut problems);
    domain::whole(&engine, &mut problems);
    defaults::document(&mut engine, &mut problems);
    if !problems.is_empty() {
        return Err(problems);
    }
    Ok(engine)
}
#[cfg(test)]
mod tests;
