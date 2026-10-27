//! Offered run policies and their reserve admission (configuration.md, 3 and 6).
use crate::{Directories, Directory, Limits, Llm, Path, Problems, Segment, What, check, read};
use alloc::boxed::Box;
use jig_domain_core::{Charter, Model, RunPolicy};
use jig_domain_fleet::Kinds;
use skein_json::Document;
use skein_lib::List;
pub(crate) fn parse(
    document: &Document,
    at: u32,
    path: &Path,
    llm: &Llm,
    limits: &Limits,
    directories: &Directories,
    problems: &mut Problems,
) -> Option<Box<[Charter]>> {
    let items = match read::list(document, at, limits.core.tasks.charters) {
        Ok(items) => items,
        Err(what) => {
            problems.push(path.clone(), what);
            return None;
        }
    };
    let mut charters = List::<Charter>::with_capacity(limits.core.tasks.charters);
    for (index, at) in items.iter().enumerate() {
        let path = path.child(Segment::Index(u32::try_from(index).expect("bounded charter count")));
        if let Some(charter) = charter(document, *at, &path, llm, limits, directories, problems) {
            for previous in &charters {
                if previous.number == charter.number {
                    problems.push(path.member(b"number"), What::Unique);
                    break;
                }
            }
            if charter.number == 0 {
                problems.push(path.member(b"number"), What::Bound);
            }
            match jig_domain_core::policy_owned_bytes(&charter.policy) {
                Some(bytes) if bytes <= u64::from(limits.core.run_bytes) => {}
                Some(_) | None => problems.push(path.clone(), What::Bound),
            }
            charters.push(charter).expect("input charter count bounded");
        }
    }
    Some(charters.into_boxed())
}
pub(crate) fn files(engine: &crate::Engine, files: &mut List<crate::File>) {
    if engine.defaults == crate::Defaults::Passed {
        return;
    }
    for (index, charter) in engine.charters.iter().enumerate() {
        let path = &charter.policy.instructions;
        let policy = jig_domain_core::policy_owned_bytes(&charter.policy).expect("validated policy size");
        let overhead = policy
            .checked_sub(u64::try_from(path.len()).expect("instruction path fits u64"))
            .expect("policy includes its instructions");
        let max =
            u64::from(engine.limits.core.run_bytes).checked_sub(overhead).expect("validated policy fits run bound");
        let at = check::index(&Path::root().member(b"charters"), index).member(b"instructions");
        crate::files::push(files, at, path, u32::try_from(max).expect("run bytes bound is u32"), false);
    }
}

fn charter(
    document: &Document,
    at: u32,
    path: &Path,
    llm: &Llm,
    limits: &Limits,
    directories: &Directories,
    problems: &mut Problems,
) -> Option<Charter> {
    let mut object = check::object(document, at, path, problems)?;
    let number = check::count(&mut object, document, b"number", path, problems);
    let instructions =
        check::path(&mut object, document, b"instructions", path, Directory::Config, directories, problems);
    let waiting = check::duration(&mut object, document, b"waiting_ms", path, problems);
    let resume = check::boolean(&mut object, document, b"resume", path, problems);
    let turns = check::count(&mut object, document, b"turns", path, problems);
    let time = check::duration(&mut object, document, b"time_ms", path, problems);
    let reserve_turns = check::count(&mut object, document, b"reserve_turns", path, problems);
    let reserve_time = check::duration(&mut object, document, b"reserve_time_ms", path, problems);
    let spend = check::integer(&mut object, document, b"spend", path, problems);
    let main = check::text(&mut object, document, b"model", path, problems);
    let model = match main {
        Some(name) => model(&name, llm, &path.member(b"model"), problems),
        None => None,
    };
    let alternatives = alternatives(&mut object, document, path, llm, problems);
    let hosts = hosts(&mut object, document, path, problems);
    let inspect = check::boolean(&mut object, document, b"inspect", path, problems);
    let modify = check::boolean(&mut object, document, b"modify", path, problems);
    let shell = check::boolean(&mut object, document, b"shell", path, problems);
    let agents = check::boolean(&mut object, document, b"agents", path, problems);
    let call_timeout = check::duration(&mut object, document, b"call_timeout_ms", path, problems);
    let documents = documents(&mut object, document, path, limits, problems);
    object.finish(document, path, problems);
    let charter = Charter {
        documents: documents?,
        number: number?,
        hosts: hosts?,
        spend: spend?,
        policy: RunPolicy {
            instructions: instructions?,
            waiting: waiting?,
            resume: resume?,
            turns: turns?,
            time: time?,
            reserve_turns: reserve_turns?,
            reserve_time: reserve_time?,
            model: model?,
            alternatives: alternatives?,
            inspect: inspect?,
            modify: modify?,
            shell: shell?,
            agents: agents?,
            call_timeout: call_timeout?,
        },
    };
    validate(&charter, path, problems);
    Some(charter)
}

fn alternatives(
    object: &mut read::Object,
    document: &Document,
    path: &Path,
    llm: &Llm,
    problems: &mut Problems,
) -> Option<Box<[Model]>> {
    let items = check::list(object, document, b"alternatives", document.len(), path, problems)?;
    let mut models = List::with_capacity(items.len());
    for (index, at) in items.iter().enumerate() {
        let path = path.member(b"alternatives").child(Segment::Index(u32::try_from(index).expect("bounded models")));
        match read::text(document, *at, 4_096) {
            Ok(name) => {
                if let Some(model) = model(&name, llm, &path, problems) {
                    for previous in &models {
                        if previous == &model {
                            problems.push(path.clone(), What::Unique);
                            break;
                        }
                    }
                    models.push(model).expect("bounded alternative count");
                }
            }
            Err(what) => problems.push(path, what),
        }
    }
    Some(models.into_boxed())
}
fn model(name: &[u8], llm: &Llm, path: &Path, problems: &mut Problems) -> Option<Model> {
    for model in &llm.models {
        if model.name.as_ref() == name {
            return Some(model.clone());
        }
    }
    problems.push(path.clone(), What::Refers);
    None
}
fn hosts(object: &mut read::Object, document: &Document, path: &Path, problems: &mut Problems) -> Option<Kinds> {
    let name = check::text(object, document, b"hosts", path, problems)?;
    match name.as_ref() {
        b"workers" => Some(Kinds::Workers),
        b"engine" => Some(Kinds::Engine),
        b"both" => Some(Kinds::Both),
        _ => {
            problems.push(path.member(b"hosts"), What::Form);
            None
        }
    }
}
fn validate(charter: &Charter, path: &Path, problems: &mut Problems) {
    let policy = &charter.policy;
    if policy.turns <= policy.reserve_turns || policy.time <= policy.reserve_time {
        problems.push(path.clone(), What::Reserve { charter: charter.number });
    }
    let enough = match jig_domain_core::reserve_floor(policy) {
        Some(floor) => charter.spend >= floor,
        None => false,
    };
    if !enough {
        problems.push(path.clone(), What::Budget { charter: charter.number });
    }
}

fn documents(
    object: &mut read::Object,
    document: &Document,
    path: &Path,
    limits: &Limits,
    problems: &mut Problems,
) -> Option<Box<[jig_domain_core::Document]>> {
    let Some(at) = object.member(document, b"documents") else {
        return Some(Box::new([]));
    };
    let path = path.member(b"documents");
    let items = crate::domain::items(document, at, limits.core.tasks.charter_documents, &path, problems)?;
    let mut result = List::with_capacity(items.len());
    for (index, at) in items.iter().enumerate() {
        let path = check::index(&path, index);
        if let Some(value) = one_document(document, *at, &path, limits, problems) {
            result.push(value).expect("bounded documents");
        }
    }
    Some(result.into_boxed())
}
fn bounded_text(document: &Document, at: u32, most: u32, path: &Path, problems: &mut Problems) -> Option<Box<[u8]>> {
    match read::text(document, at, most) {
        Ok(value) => Some(value),
        Err(what) => {
            problems.push(path.clone(), what);
            None
        }
    }
}
fn one_document(
    document: &Document,
    at: u32,
    path: &Path,
    limits: &Limits,
    problems: &mut Problems,
) -> Option<jig_domain_core::Document> {
    let mut object = check::object(document, at, path, problems)?;
    let connector = check::short(&mut object, document, b"connector", path, problems);
    let names = check::list(&mut object, document, b"name", limits.core.authority.segments, path, problems);
    let mut name = List::with_capacity(limits.core.authority.segments);
    if let Some(names) = names {
        for (index, at) in names.iter().enumerate() {
            let path = check::index(&path.member(b"name"), index);
            if let Some(value) = bounded_text(document, *at, limits.core.authority.segment_bytes, &path, problems) {
                name.push(value).expect("bounded name");
            }
        }
    }
    let at = object.required(document, b"path", path, problems)?;
    let file = bounded_text(document, at, 256, &path.member(b"path"), problems);
    let heading = match object.member(document, b"heading") {
        Some(at) => Some(bounded_text(document, at, 80, &path.member(b"heading"), problems)?),
        None => None,
    };
    object.finish(document, path, problems);
    Some(jig_domain_core::Document { connector: connector?, name: name.into_boxed(), path: file?, heading })
}
