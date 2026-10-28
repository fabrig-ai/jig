//! Parse and check everything before constructing a running service (service.md, 3).
use crate::{Agent, Engine, Files, Limits, Seeds, Service, limits, system::System};
use alloc::boxed::Box;
use jig_protocol_config::{self as config, Path, Problems, Profile, Segment, What, read};
use jig_test_connector as connector;
use jig_test_domain as root;
use skein_json::Document;
use skein_lib::{List, bytes};

type ServerConfiguration = (skein_io::Addr, Option<Box<[u8]>>);

type Initial = Box<[(connector::Path, u64)]>;

const ACCOUNT_APPLICATION: &[u8] = b"jig-test";

#[derive(Debug)]
pub(super) struct Parsed {
    pub(super) engine: config::Engine,
    pub(super) limits: Limits,
    pub(super) first: connector::Config,
    pub(super) second: connector::Config,
    initial: [Initial; 2],
    credential: Box<[u8]>,
    accounts: bool,
    workers: bool,
    clients: bool,
    listener: skein_io::Addr,
    bundle: Option<Box<[u8]>>,
}

fn problem(path: Path, what: What) -> Problems {
    let mut problems = Problems::empty();
    problems.push(path, what);
    problems
}
fn engine_path(path: &Path) -> Path {
    let mut complete = Path::root().member(b"engine");
    for segment in &path.segments {
        complete = complete.child(segment.clone());
    }
    complete
}

fn object(document: &Document, at: u32, path: &Path) -> Result<read::Object, Problems> {
    match read::Object::new(document, at) {
        Ok(value) => Ok(value),
        Err(what) => Err(problem(path.clone(), what)),
    }
}

fn text(document: &Document, at: u32, max: u32, path: &Path) -> Result<Box<[u8]>, Problems> {
    match read::text(document, at, max) {
        Ok(value) => Ok(value),
        Err(what) => Err(problem(path.clone(), what)),
    }
}
fn list(document: &Document, at: u32, max: u32, path: &Path) -> Result<List<u32>, Problems> {
    match read::list(document, at, max) {
        Ok(value) => Ok(value),
        Err(what) => Err(problem(path.clone(), what)),
    }
}
fn integer(document: &Document, at: u32, path: &Path) -> Result<u64, Problems> {
    match read::integer(document, at, u64::MAX) {
        Ok(value) => Ok(value),
        Err(what) => Err(problem(path.clone(), what)),
    }
}

fn components(
    document: &Document,
    whole: &mut read::Object,
    problems: &mut Problems,
) -> Result<(bool, bool, bool), Problems> {
    let mut accounts = false;
    let mut workers = false;
    let mut clients = false;
    if let Some(at) = whole.member(document, b"service") {
        let here = Path::root().member(b"service");
        let mut service = object(document, at, &here)?;
        if let Some(at) = service.member(document, b"components") {
            let items = list(document, at, document.len(), &here.member(b"components"))?;
            for (index, at) in items.iter().enumerate() {
                let here = indexed(&here.member(b"components"), index);
                let name = text(document, *at, 32, &here)?;
                let what = match name.as_ref() {
                    b"accounts" if !accounts => {
                        accounts = true;
                        continue;
                    }
                    b"workers" if !workers => {
                        workers = true;
                        continue;
                    }
                    b"clients" if !clients => {
                        clients = true;
                        continue;
                    }
                    b"accounts" | b"workers" | b"clients" => What::Duplicate,
                    _ => What::Unknown,
                };
                problems.push(here, what);
            }
        }
        service.finish(document, &here, problems);
    }
    Ok((accounts, workers, clients))
}

pub(super) fn parse(input: &[u8]) -> Result<Parsed, Problems> {
    let length = u32::try_from(input.len()).or(Err(problem(Path::root(), What::Bound)))?;
    let bounds = config::DocumentLimits { bytes: length, tokens: length, depth: 32 };
    let document = match config::document(input, &bounds) {
        Ok(value) => value,
        Err(found) => return Err(problem(found.path, found.what)),
    };
    parse_in(&document)
}

fn link_enabled(document: &Document, at: u32) -> bool {
    let Ok(mut engine) = read::Object::new(document, at) else { return false };
    let Some(at) = engine.member(document, b"clients") else { return false };
    let Ok(mut clients) = read::Object::new(document, at) else { return false };
    let Some(at) = clients.member(document, b"link") else { return false };
    read::boolean(document, at) == Ok(true)
}

fn parse_in(document: &Document) -> Result<Parsed, Problems> {
    let length = document.text_len();
    let mut problems = Problems::empty();
    let path = Path::root();
    let mut whole = object(document, 0, &path)?;
    let at = whole.required(document, b"engine", &path, &mut problems);
    let application = whole.required(document, b"application", &path, &mut problems);
    if let Some(at) = whole.member(document, b"workers") {
        let items = list(document, at, document.len(), &path.member(b"workers"))?;
        drop(items);
    }
    let (accounts, workers, clients) = components(document, &mut whole, &mut problems)?;
    whole.finish(document, &path, &mut problems);
    if !problems.is_empty() {
        return Err(problems);
    }
    let at = at.expect("required engine checked");
    let application_path = Path::root().member(b"application");
    let mut application = object(document, application.expect("required application checked"), &application_path)?;
    let (listener, bundle) = server(document, &mut application, link_enabled(document, at), &mut problems)?;
    let engine = match config::engine(document, at, &config::Directories::working(), config::Defaults::Document) {
        Ok(value) => value,
        Err(found) => {
            for found in &found.problems {
                problems.push(engine_path(&found.path), found.what);
            }
            problems.past = problems.past.saturating_add(found.past);
            return Err(problems);
        }
    };
    if engine.clients.provider.is_some() {
        return Err(problem(Path::root().member(b"engine").member(b"clients").member(b"provider"), What::Form));
    }
    let mut engine_object = object(document, at, &path.member(b"engine"))?;
    let at = engine_object.member(document, b"limits").expect("engine parser checked limits");
    let mut profile = object(document, at, &path)?;
    let at = profile.member(document, b"profile").expect("engine parser checked profile");
    let profile = text(document, at, 16, &path)?;
    let profile = match profile.as_ref() {
        b"small" => Profile::Small,
        b"standard" => Profile::Standard,
        _ => return Err(problem(path, What::Form)),
    };
    let limits = limits::for_profile(profile, &engine.limits);
    let path = Path::root().member(b"application");
    let _workers = application.member(document, b"workers");
    let at = application.required(document, b"credential", &path, &mut problems);
    let credential = match at {
        Some(at) => text(document, at, length, &path.member(b"credential"))?,
        None => Box::default(),
    };
    let at = application.required(document, b"connectors", &path, &mut problems);
    let items = match at {
        Some(at) => list(document, at, 2, &path.member(b"connectors"))?,
        None => List::with_capacity(0),
    };
    if items.len() != 2 {
        problems.push(path.member(b"connectors"), What::Bound);
    }
    application.finish(document, &path, &mut problems);
    if !problems.is_empty() {
        return Err(problems);
    }
    let first = connector_config(document, *items.get(0).expect("two connectors"), 0, &limits.root.connector)?;
    let second = connector_config(document, *items.get(1).expect("two connectors"), 1, &limits.root.connector)?;
    Ok(Parsed {
        engine,
        limits,
        first: first.0,
        second: second.0,
        initial: [first.1, second.1],
        credential,
        accounts,
        workers,
        clients,
        listener,
        bundle,
    })
}

/// The application's listener is loopback until a TLS server is available (service.md, 7).
fn server(
    document: &Document,
    application: &mut read::Object,
    link: bool,
    problems: &mut Problems,
) -> Result<ServerConfiguration, Problems> {
    let path = Path::root().member(b"application").member(b"server");
    let at = application.required(document, b"server", &Path::root().member(b"application"), problems);
    let Some(at) = at else { return Err(core::mem::replace(problems, Problems::empty())) };
    let mut fields = object(document, at, &path)?;
    let at = fields.required(document, b"listener", &path, problems);
    let address = match at {
        Some(at) => config::address::read(document, at, &path.member(b"listener"), problems),
        None => None,
    };
    let bundle = match fields.member(document, b"bundle") {
        Some(at) => Some(text(document, at, 4096, &path.member(b"bundle"))?),
        None => None,
    };
    fields.finish(document, &path, problems);
    let listener = match address {
        Some(config::Address::Addr(addr)) if loopback(&addr) => Some(addr),
        Some(config::Address::Addr(_) | config::Address::Host { .. }) => {
            problems.push(path.member(b"listener"), What::Plaintext);
            if link {
                problems.push(Path::root().member(b"engine").member(b"clients").member(b"link"), What::Plaintext);
            }
            None
        }
        None => None,
    };
    match listener {
        Some(listener) if problems.is_empty() => Ok((listener, bundle)),
        Some(_) | None => Err(core::mem::replace(problems, Problems::empty())),
    }
}

fn loopback(addr: &skein_io::Addr) -> bool {
    match addr.socket() {
        Some(socket) => socket.ip().is_loopback(),
        None => false,
    }
}

fn indexed(path: &Path, index: usize) -> Path {
    path.child(Segment::Index(u32::try_from(index).expect("bounded list")))
}
fn resource(
    document: &Document,
    at: u32,
    path: &Path,
    limits: &connector::Limits,
) -> Result<connector::Path, Problems> {
    let items = list(document, at, limits.path_segments, path)?;
    let mut parts = List::with_capacity(limits.path_segments);
    for at in &items {
        parts.push(text(document, *at, limits.segment_bytes, path)?).expect("bounded path");
    }
    connector::Path::new(parts, limits.segment_bytes).ok_or(problem(path.clone(), What::Bound))
}

fn connector_config(
    document: &Document,
    at: u32,
    index: usize,
    limits: &connector::Limits,
) -> Result<(connector::Config, Initial), Problems> {
    let path = indexed(&Path::root().member(b"application").member(b"connectors"), index);
    let mut problems = Problems::empty();
    let mut fields = object(document, at, &path)?;
    let prefix = fields.required(document, b"prefix", &path, &mut problems);
    let resources = fields.required(document, b"resources", &path, &mut problems);
    fields.finish(document, &path, &mut problems);
    if !problems.is_empty() {
        return Err(problems);
    }
    let prefix = resource(document, prefix.expect("prefix checked"), &path.member(b"prefix"), limits)?;
    let items = list(document, resources.expect("resources checked"), limits.resources, &path.member(b"resources"))?;
    let mut resources = List::<connector::ResourceSpec>::with_capacity(limits.resources);
    let mut initial = List::with_capacity(limits.resources);
    for (index, at) in items.iter().enumerate() {
        let here = indexed(&path.member(b"resources"), index);
        let mut row = object(document, *at, &here)?;
        let name = row.required(document, b"path", &here, &mut problems);
        let state = row.required(document, b"state", &here, &mut problems);
        row.finish(document, &here, &mut problems);
        if !problems.is_empty() {
            return Err(problems);
        }
        let name = resource(document, name.expect("path checked"), &here.member(b"path"), limits)?;
        let state = integer(document, state.expect("state checked"), &here.member(b"state"))?;
        if !name.under(&prefix) {
            return Err(problem(here.member(b"path"), What::Refers));
        }
        for previous in &resources {
            if previous.path == name {
                return Err(problem(here.member(b"path"), What::Duplicate));
            }
        }
        initial.push((name.clone(), state)).expect("resource count checked");
        resources
            .push(connector::ResourceSpec {
                path: name,
                credential: Some(1),
                hold: connector::Hold::None,
                writable: true,
            })
            .expect("resource count checked");
    }
    let config = connector::Config {
        deployment: [0; 16],
        prefix,
        resources: resources.into_boxed(),
        pools: Box::default(),
        topics: Box::default(),
        kinds: Box::new([connector::KindSpec {
            waits_for_dependencies: false,
            kind: 1,
            form: connector::Form::Set,
            recovery: connector::Recovery::Keyed,
            price: None,
        }]),
        requirements: Box::default(),
        procedures: Box::default(),
    };
    Ok((config, initial.into_boxed()))
}

fn engine_named(document: &[u8]) -> Result<List<Box<[u8]>>, Problems> {
    let parsed = parse(document)?;
    let files = config::files(&parsed.engine);
    let mut names =
        List::with_capacity(u32::try_from(files.len()).expect("file count").checked_add(1).expect("credential file"));
    for file in files {
        if !names.as_slice().contains(&file.path) {
            names.push(file.path).expect("file count");
        }
    }
    if !names.as_slice().contains(&parsed.credential) {
        names.push(parsed.credential).expect("credential file");
    }
    Ok(names)
}

fn check_files(parsed: &Parsed, files: &Files) -> Result<(), Problems> {
    let mut problems = Problems::empty();
    for file in config::files(&parsed.engine) {
        let mut found = None;
        for (name, contents) in &files.entries {
            if *name == file.path {
                found = Some(contents.expose());
            }
        }
        match found {
            Some(contents) if contents.len() <= usize::try_from(file.max).expect("file bound") => {}
            Some(_) => problems.push(engine_path(&file.at), What::Bound),
            None => problems.push(engine_path(&file.at), What::Unreadable),
        }
    }
    let mut credential = false;
    for (name, _) in &files.entries {
        credential |= *name == parsed.credential;
    }
    if !credential {
        problems.push(Path::root().member(b"application").member(b"credential"), What::Unreadable);
    }
    if !problems.is_empty() {
        return Err(problems);
    }
    Ok(())
}

/// The application's credential table: generation 1 from its file, room for two generations of each number.
fn application(parsed: &Parsed, files: &Files) -> Result<skein_lib::Map<(u32, u64), skein_lib::Secret>, Problems> {
    let mut application = skein_lib::Map::with_capacity(
        parsed.limits.root.core.accounts.credentials.checked_mul(2).expect("two application generations"),
    );
    let credential = files.contents(&parsed.credential).expect("credential file checked");
    if credential.len() > usize::try_from(parsed.limits.root.connector.value_bytes).expect("credential bound") {
        return Err(problem(Path::root().member(b"application").member(b"credential"), What::Bound));
    }
    application.insert((1, 1), config::secret(credential.into())).expect("one application credential");
    Ok(application)
}

pub(super) fn engine_startup(
    mut parsed: Parsed,
    files: &Files,
    _agent: Option<&Agent>,
    seeds: &Seeds,
    runtime: Option<crate::AccountsRuntime>,
    own_workers: bool,
) -> Result<(Engine, skein_io::Limits, u64), Problems> {
    parsed.workers |= own_workers;
    check_files(&parsed, files)?;
    for charter in &mut parsed.engine.charters {
        for (name, contents) in &files.entries {
            if *name == charter.policy.instructions {
                charter.policy.instructions = bytes::copy_of(contents.expose());
                break;
            }
        }
    }
    let store_limits = configured_store_limits(&parsed)?;
    parsed.limits.store = store_limits;
    let trust = match &runtime {
        Some(runtime) if runtime.trust_roots > 0 => {
            skein_tls::roots_worst_case(runtime.trust_roots, runtime.trust_root_bytes)
        }
        Some(_) | None => Some(0),
    }
    .ok_or(problem(Path::root().member(b"engine").member(b"memory_bytes"), What::Bound))?;
    let configured = configuration_bytes(&parsed.engine)?;
    let bytes = limits::worst_case(&parsed.limits)
        .ok_or(problem(Path::root().member(b"engine").member(b"memory_bytes"), What::Bound))?
        .checked_add(trust)
        .ok_or(problem(Path::root().member(b"engine").member(b"memory_bytes"), What::Bound))?;
    let bytes = bytes
        .checked_add(configured)
        .ok_or(problem(Path::root().member(b"engine").member(b"memory_bytes"), What::Bound))?;
    parsed.first.deployment = seeds.deployment;
    parsed.second.deployment = seeds.deployment;
    let max = root::max_out(&parsed.limits.root);
    let accounts = if parsed.accounts {
        Some(crate::accounts::Accounts::new(
            ACCOUNT_APPLICATION.into(),
            &parsed.engine,
            seeds,
            runtime,
            max.checked_mul(6).expect("account routing room"),
            &store_limits.secrets,
        )?)
    } else {
        None
    };
    let workers = if parsed.workers {
        Some(crate::workers::Workers::new(
            &parsed.engine,
            files,
            seeds.deployment,
            &parsed.limits.workers,
            max.checked_mul(2)
                .expect("engine routes")
                .checked_mul(jig_protocol_engine_workers::MAX_OUT_DOWN)
                .expect("worker routing room"),
        )?)
    } else {
        None
    };
    let clients = clients_child(&parsed, seeds, max)?;
    let values = crate::values::Values {
        application: application(&parsed, files)?,
        accounts: jig_protocol_accounts::table::Table::new(
            parsed.limits.accounts.accounts,
            parsed.limits.accounts.client.document.token_bytes,
        ),
    };
    let core = config::core(&parsed.engine, seeds.deployment, seeds.seed);
    let root = root::Config { core, first: parsed.first, second: parsed.second };
    let mut first_periods = List::with_capacity(parsed.limits.root.core.authority.projects);
    for project in &root.core.seed_projects {
        first_periods.push((project.number, project.policy.policy.period_spend)).expect("seed project bound");
    }
    let connector = parsed.limits.root.connector;
    let mut systems =
        [System::new(connector.resources, connector.made), System::new(connector.resources, connector.made)];
    for (index, initial) in parsed.initial.into_iter().enumerate() {
        let system = systems.get_mut(index).expect("two systems");
        for (resource, state) in initial {
            system.seed(resource, state).expect("configuration checked resource count");
        }
    }
    let store = crate::store::Store::new(
        &parsed.limits.root,
        &store_limits,
        &root.core.settings.models,
        seeds.installation,
        seeds.seed,
    );
    let mut engine = Engine::new(
        root,
        &parsed.limits.root,
        systems,
        crate::engine::Children { accounts, workers, clients },
        values,
        store,
    );
    // configuration.md, 5: the smallest source opens only an empty deployment.
    engine.first_periods = first_periods;
    Ok((engine, parsed.limits.io, bytes))
}

fn clients_child(parsed: &Parsed, seeds: &Seeds, max: u32) -> Result<Option<crate::clients::Clients>, Problems> {
    Ok(if parsed.clients {
        Some(crate::clients::Clients::new(
            &parsed.engine,
            parsed.listener,
            seeds,
            max.checked_mul(2)
                .expect("engine routes")
                .checked_mul(jig_protocol_engine_clients::MAX_OUT_DOWN)
                .expect("client routing room"),
            &parsed.limits.server,
        )?)
    } else {
        None
    })
}

#[derive(Debug)]
struct WorkerSpec {
    config: config::Worker,
    root: Box<[u8]>,
}
fn document(input: &[u8]) -> Result<Document, Problems> {
    let length = u32::try_from(input.len()).or(Err(problem(Path::root(), What::Bound)))?;
    match config::document(input, &config::DocumentLimits { bytes: length, tokens: length, depth: 32 }) {
        Ok(document) => Ok(document),
        Err(found) => Err(problem(found.path, found.what)),
    }
}

fn worker_specs(input: &[u8]) -> Result<(bool, List<WorkerSpec>), Problems> {
    worker_specs_in(&document(input)?)
}

fn worker_specs_in(document: &Document) -> Result<(bool, List<WorkerSpec>), Problems> {
    let path = Path::root();
    let mut whole = object(document, 0, &path)?;
    let engine_at = whole.member(document, b"engine");
    let engine = engine_at.is_some();
    let application = whole.member(document, b"application");
    let workers = match whole.member(document, b"workers") {
        Some(at) => list(document, at, document.len(), &path.member(b"workers"))?,
        None => List::with_capacity(0),
    };
    let listeners = match engine_at {
        Some(_) if !workers.is_empty() => parse_in(document)?.engine.hosts.listener,
        Some(_) | None => Box::default(),
    };
    let mut problems = Problems::empty();
    if !engine {
        let _service = components(document, &mut whole, &mut problems)?;
        whole.finish(document, &path, &mut problems);
    }
    let root_path = path.member(b"application").member(b"workers");
    let roots = match application {
        Some(at) => {
            let mut fields = object(document, at, &path.member(b"application"))?;
            let roots = match fields.member(document, b"workers") {
                Some(at) => list(document, at, workers.len(), &root_path)?,
                None => List::with_capacity(0),
            };
            if !engine {
                fields.finish(document, &path.member(b"application"), &mut problems);
            }
            roots
        }
        None => List::with_capacity(0),
    };
    if workers.len() != roots.len() {
        problems.push(root_path.clone(), What::Missing);
    }
    if !engine && workers.is_empty() {
        problems.push(path.member(b"engine"), What::Missing);
    }
    if !problems.is_empty() {
        return Err(problems);
    }
    let mut specs = List::with_capacity(workers.len());
    for (index, at) in workers.iter().enumerate() {
        let here = indexed(&path.member(b"workers"), index);
        let worker = match config::worker(document, *at, &config::Directories::working()) {
            Ok(worker) => worker,
            Err(found) => {
                for found in &found.problems {
                    let mut complete = here.clone();
                    for segment in &found.path.segments {
                        complete = complete.child(segment.clone());
                    }
                    problems.push(complete, found.what);
                }
                return Err(problems);
            }
        };
        if let config::Address::Addr(skein_io::Addr::InProcess(_)) = &worker.engine
            && !listeners.contains(&worker.engine)
        {
            return Err(problem(here.member(b"engine"), What::Refers));
        }
        let here = indexed(&root_path, index);
        let mut fields =
            object(document, *roots.get(u32::try_from(index).expect("worker index")).expect("root count"), &here)?;
        let at = fields.required(document, b"root", &here, &mut problems);
        fields.finish(document, &here, &mut problems);
        if !problems.is_empty() {
            return Err(problems);
        }
        let root = text(document, at.expect("root checked"), 4096, &here.member(b"root"))?;
        specs.push(WorkerSpec { config: worker, root }).expect("worker count");
    }
    Ok((engine, specs))
}

pub fn named(document: &[u8]) -> Result<List<Box<[u8]>>, Problems> {
    let (engine, specs) = worker_specs(document)?;
    let names = if engine { engine_named(document)? } else { List::with_capacity(0) };
    let capacity = names.len().checked_add(specs.len().checked_mul(2).expect("worker files")).expect("named files");
    let mut files = List::with_capacity(capacity);
    for name in names.into_boxed() {
        files.push(name).expect("named files");
    }
    for spec in specs.into_boxed() {
        for name in [spec.config.secret, spec.config.agent.configuration] {
            if !files.as_slice().contains(&name) {
                files.push(name).expect("named files");
            }
        }
    }
    Ok(files)
}

pub fn startup(
    document: &[u8],
    files: &Files,
    agent: Option<&Agent>,
    seeds: &Seeds,
    accounts: Option<crate::AccountsRuntime>,
    runtime: Option<crate::WorkerRuntime>,
) -> Result<Service, Problems> {
    let document = self::document(document)?;
    let (has_engine, specs) = worker_specs_in(&document)?;
    let mut bundle = None;
    let engine = if has_engine {
        let mut parsed = parse_in(&document)?;
        bundle = parsed.bundle.take();
        Some(engine_startup(parsed, files, agent, seeds, accounts, !specs.is_empty())?)
    } else {
        None
    };
    let mut io = match &engine {
        Some((_, io, _)) => *io,
        None => {
            let mut io = specs.get(0).expect("engine or worker").config.limits.io;
            io.sockets = 0;
            io.names = 0;
            io
        }
    };
    let mut bytes = match &engine {
        Some((_, _, bytes)) => *bytes,
        None => 0,
    };
    let mut workers = List::with_capacity(specs.len());
    let mut queue = match &engine {
        Some((engine, _, _)) => engine.max_out.checked_mul(2).expect("service queue"),
        None => 1,
    };
    for (index, spec) in specs.into_boxed().into_iter().enumerate() {
        let path = indexed(&Path::root().member(b"application").member(b"workers"), index).member(b"root");
        let index = u32::try_from(index).expect("worker index");
        let runtime = runtime.as_ref().ok_or(problem(path.clone(), What::Refers))?;
        let directory = *runtime.roots.get(index).ok_or(problem(path.clone(), What::Refers))?;
        for prior in 0..index {
            if runtime.roots.get(prior) == Some(&directory) {
                return Err(problem(path.clone(), What::Duplicate));
            }
        }
        let agent = agent.ok_or(problem(
            Path::root().member(b"workers").child(Segment::Index(index)).member(b"agent"),
            What::Refers,
        ))?;
        if files.contents(&spec.config.agent.configuration).is_none() {
            return Err(problem(
                Path::root().member(b"workers").child(Segment::Index(index)).member(b"agent").member(b"configuration"),
                What::Unreadable,
            ));
        }
        let bound = crate::worker::worst_case(&spec.config, agent, spec.root.len())
            .ok_or(problem(path.clone(), What::Bound))?;
        bytes = bytes.checked_add(bound).ok_or(problem(path.clone(), What::Bound))?;
        io.sockets = io.sockets.checked_add(spec.config.limits.io.sockets).ok_or(problem(path.clone(), What::Bound))?;
        io.names = io.names.checked_add(spec.config.limits.io.names).ok_or(problem(path.clone(), What::Bound))?;
        io.intake = io.intake.max(spec.config.limits.io.intake);
        io.receive = io.receive.max(spec.config.limits.io.receive);
        io.output = io.output.max(spec.config.limits.io.output);
        io.sends = io.sends.max(spec.config.limits.io.sends);
        let worker = crate::Worker::new(spec.config, agent, files, directory, index, seeds.seed)?;
        queue = queue.max(worker.queue_room());
        workers.push(worker).expect("worker count");
    }
    if !workers.is_empty() {
        bytes = bytes
            .checked_add(
                limits::shared_worker_bytes(&io, queue, workers.len()).ok_or(problem(Path::root(), What::Bound))?,
            )
            .ok_or(problem(Path::root(), What::Bound))?;
    }
    let mut service = match engine {
        Some((engine, _, _)) => Service::new(Some(engine), workers, io, bytes, queue),
        None => Service::new(None, workers, io, bytes, queue),
    };
    service.bundle_directory = bundle;
    Ok(service)
}

#[cfg(test)]
pub(super) fn fixture_connectors(document: &[u8]) -> (config::Engine, [connector::Config; 2]) {
    let parsed = parse(document).expect("fixture configuration");
    (parsed.engine, [parsed.first, parsed.second])
}

fn configuration_bytes(engine: &config::Engine) -> Result<u64, Problems> {
    let workers = crate::workers::configured_bytes(engine)
        .ok_or(problem(Path::root().member(b"engine").member(b"memory_bytes"), What::Bound))?;
    let accounts = crate::accounts::configured_bytes(engine)
        .ok_or(problem(Path::root().member(b"engine").member(b"memory_bytes"), What::Bound))?;
    workers.checked_add(accounts).ok_or(problem(Path::root().member(b"engine").member(b"memory_bytes"), What::Bound))
}

/// The store reserves the same number of held values as the clients' carried table.
fn configured_store_limits(parsed: &Parsed) -> Result<jig_protocol_store::Limits, Problems> {
    let mut limits = parsed.limits.store;
    limits.held = parsed
        .limits
        .clients
        .exchanges
        .checked_mul(2)
        .ok_or(problem(Path::root().member(b"engine").member(b"limits"), What::Bound))?;
    crate::store::limits(&parsed.limits.root, &limits)
        .ok_or(problem(Path::root().member(b"engine").member(b"limits"), What::Bound))
}
