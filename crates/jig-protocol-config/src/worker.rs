//! A worker's startup values and its bounded end (configuration.md, 4; channel.md, 10.1).
use crate::{Address, Directories, Directory, Limits, Path, Problems, Profile, What, check, overrides, profile, read};
use alloc::boxed::Box;
use skein_json::Document;
use skein_lib::{Duration, List};

#[derive(Debug)]
pub struct Worker {
    pub engine: Address,
    pub name: Box<[u8]>,
    /// The private file holding the worker's secret, never the secret itself.
    pub secret: Box<[u8]>,
    pub slots: u32,
    pub graces: Graces,
    pub stop_bound: Duration,
    pub agent: Agent,
    /// Positive interval between local source-login observations.
    pub check_interval: Duration,
    pub limits: Limits,
    pub memory: Option<u64>,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Graces {
    pub grace: Duration,
    pub agent_stop: Duration,
    pub delivery: Duration,
    pub save: Duration,
}
/// The program and the file smith's startup parses; jig does not read that document.
#[derive(Debug)]
pub struct Agent {
    pub program: Box<[u8]>,
    pub arguments: Box<[Box<[u8]>]>,
    pub configuration: Box<[u8]>,
}
/// The bound stated at hello, measured from the engine's last possible frame.
#[must_use]
pub fn stop_bound(graces: &Graces, ping: Duration, silence: Duration) -> Option<Duration> {
    ping.checked_add(silence)?
        .checked_add(graces.grace)?
        .checked_add(graces.agent_stop.max(graces.delivery))?
        .checked_add(graces.save)
}
/// Parse jig's worker member before startup constructs any component.
pub fn worker(document: &Document, at: u32, directories: &Directories) -> Result<Worker, Problems> {
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
    let engine = check::address(&mut object, document, b"engine", &path, &mut problems);
    if let Some(engine) = &engine {
        check::local(engine, &path.member(b"engine"), &mut problems);
    }
    let name = check::text(&mut object, document, b"name", &path, &mut problems);
    if let Some(name) = &name
        && name.len() > usize::try_from(limits.link.name).expect("name bound fits usize")
    {
        problems.push(path.member(b"name"), What::Bound);
    }
    let secret = check::path(&mut object, document, b"secret", &path, Directory::Data, directories, &mut problems);
    let slots = check::count(&mut object, document, b"slots", &path, &mut problems);
    if let Some(slots) = slots
        && slots > limits.hub.slots
    {
        problems.push(path.member(b"slots"), What::Bound);
    }
    let mut graces = None;
    if let Some(at) = object.required(document, b"graces", &path, &mut problems) {
        graces = parse_graces(document, at, &path.member(b"graces"), &mut problems);
    }
    let mut check_interval = None;
    if let Some(at) = object.required(document, b"programs", &path, &mut problems) {
        let path = path.member(b"programs");
        if let Some(mut programs) = check::object(document, at, &path, &mut problems) {
            check_interval = check::duration(&mut programs, document, b"check_interval_ms", &path, &mut problems);
            if check_interval == Some(Duration::ZERO) {
                problems.push(path.member(b"check_interval_ms"), What::Bound);
            }
            programs.finish(document, &path, &mut problems);
        }
    }
    if let Some(interval) = check_interval {
        limits.hub.source_interval = interval;
    }
    let mut agent = None;
    if let Some(at) = object.required(document, b"agent", &path, &mut problems) {
        agent = parse_agent(document, at, &path.member(b"agent"), directories, &mut problems);
    }
    let memory = check::optional_integer(&mut object, document, b"memory_bytes", &path, &mut problems);
    object.finish(document, &path, &mut problems);
    let bound = check::worker(&limits, graces, &mut problems);
    if !problems.is_empty() {
        return Err(problems);
    }
    Ok(Worker {
        engine: engine.expect("required member checked"),
        name: name.expect("required member checked"),
        secret: secret.expect("required member checked"),
        slots: slots.expect("required member checked"),
        graces: graces.expect("required member checked"),
        stop_bound: bound.expect("duration sum checked"),
        agent: agent.expect("required member checked"),
        check_interval: check_interval.expect("required member checked"),
        limits,
        memory,
    })
}
fn parse_graces(document: &Document, at: u32, path: &Path, problems: &mut Problems) -> Option<Graces> {
    let mut object = check::object(document, at, path, problems)?;
    let grace = check::duration(&mut object, document, b"grace_ms", path, problems);
    let agent_stop = check::duration(&mut object, document, b"agent_stop_ms", path, problems);
    let delivery = check::duration(&mut object, document, b"delivery_ms", path, problems);
    let save = check::duration(&mut object, document, b"save_ms", path, problems);
    object.finish(document, path, problems);
    Some(Graces { grace: grace?, agent_stop: agent_stop?, delivery: delivery?, save: save? })
}
fn parse_agent(
    document: &Document,
    at: u32,
    path: &Path,
    directories: &Directories,
    problems: &mut Problems,
) -> Option<Agent> {
    let mut object = check::object(document, at, path, problems)?;
    let program = check::text(&mut object, document, b"program", path, problems);
    let configuration =
        check::path(&mut object, document, b"configuration", path, Directory::Config, directories, problems);
    let mut arguments = List::with_capacity(document.len());
    if let Some(items) = check::list(&mut object, document, b"arguments", document.len(), path, problems) {
        for (index, at) in items.iter().enumerate() {
            match read::text(document, *at, 4_096) {
                Ok(value) => arguments.push(value).expect("arguments bounded by document tokens"),
                Err(what) => problems.push(check::index(&path.member(b"arguments"), index), what),
            }
        }
    }
    object.finish(document, path, problems);
    Some(Agent { program: program?, arguments: arguments.into_boxed(), configuration: configuration? })
}
