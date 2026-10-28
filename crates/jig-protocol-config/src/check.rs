//! Collected field and reference checks (configuration.md, 1 and 6).
use crate::{Address, Directories, Directory, Path, Problems, What, address, read};
use alloc::boxed::Box;
use skein_io::Addr;
use skein_json::Document;
use skein_lib::{Duration, List};
pub(crate) fn object(document: &Document, at: u32, path: &Path, problems: &mut Problems) -> Option<read::Object> {
    match read::Object::new(document, at) {
        Ok(object) => Some(object),
        Err(what) => {
            problems.push(path.clone(), what);
            None
        }
    }
}
pub(crate) fn text(
    object: &mut read::Object,
    document: &Document,
    name: &[u8],
    path: &Path,
    problems: &mut Problems,
) -> Option<Box<[u8]>> {
    let at = object.required(document, name, path, problems)?;
    match read::text(document, at, 4_096) {
        Ok(value) => Some(value),
        Err(what) => {
            problems.push(path.member(name), what);
            None
        }
    }
}
pub(crate) fn path(
    object: &mut read::Object,
    document: &Document,
    name: &[u8],
    path: &Path,
    directory: Directory,
    directories: &Directories,
    problems: &mut Problems,
) -> Option<Box<[u8]>> {
    let value = text(object, document, name, path, problems)?;
    match directories.join(directory, &value) {
        Ok(value) => Some(value),
        Err(what) => {
            problems.push(path.member(name), what);
            None
        }
    }
}
pub(crate) fn optional_path(
    object: &mut read::Object,
    document: &Document,
    name: &[u8],
    path: &Path,
    directory: Directory,
    directories: &Directories,
    problems: &mut Problems,
) -> Option<Box<[u8]>> {
    let at = object.member(document, name)?;
    let value = match read::text(document, at, 4_096) {
        Ok(value) => value,
        Err(what) => {
            problems.push(path.member(name), what);
            return None;
        }
    };
    match directories.join(directory, &value) {
        Ok(value) => Some(value),
        Err(what) => {
            problems.push(path.member(name), what);
            None
        }
    }
}
pub(crate) fn integer(
    object: &mut read::Object,
    document: &Document,
    name: &[u8],
    path: &Path,
    problems: &mut Problems,
) -> Option<u64> {
    let at = object.required(document, name, path, problems)?;
    match read::integer(document, at, u64::MAX) {
        Ok(value) => Some(value),
        Err(what) => {
            problems.push(path.member(name), what);
            None
        }
    }
}
pub(crate) fn optional_integer(
    object: &mut read::Object,
    document: &Document,
    name: &[u8],
    path: &Path,
    problems: &mut Problems,
) -> Option<u64> {
    let at = object.member(document, name)?;
    match read::integer(document, at, u64::MAX) {
        Ok(value) => Some(value),
        Err(what) => {
            problems.push(path.member(name), what);
            None
        }
    }
}
pub(crate) fn count(
    object: &mut read::Object,
    document: &Document,
    name: &[u8],
    path: &Path,
    problems: &mut Problems,
) -> Option<u32> {
    let value = integer(object, document, name, path, problems)?;
    match u32::try_from(value) {
        Ok(value) => Some(value),
        Err(_) => {
            problems.push(path.member(name), What::Bound);
            None
        }
    }
}
pub(crate) fn short(
    object: &mut read::Object,
    document: &Document,
    name: &[u8],
    path: &Path,
    problems: &mut Problems,
) -> Option<u16> {
    let value = integer(object, document, name, path, problems)?;
    match u16::try_from(value) {
        Ok(value) => Some(value),
        Err(_) => {
            problems.push(path.member(name), What::Bound);
            None
        }
    }
}
pub(crate) fn optional(
    object: &mut read::Object,
    document: &Document,
    name: &[u8],
    path: &Path,
    problems: &mut Problems,
) -> Result<Option<u64>, What> {
    let at = object.required(document, name, path, problems).ok_or(What::Missing)?;
    if document.token(at).ok_or(What::Kind)?.kind == skein_json::document::Kind::Null {
        return Ok(None);
    }
    match read::integer(document, at, u64::MAX) {
        Ok(value) => Ok(Some(value)),
        Err(what) => {
            problems.push(path.member(name), what);
            Err(what)
        }
    }
}
pub(crate) fn index(path: &Path, index: usize) -> Path {
    path.child(crate::Segment::Index(u32::try_from(index).expect("document count fits u32")))
}
pub(crate) fn duration(
    object: &mut read::Object,
    document: &Document,
    name: &[u8],
    path: &Path,
    problems: &mut Problems,
) -> Option<Duration> {
    let at = object.required(document, name, path, problems)?;
    match read::duration(document, at) {
        Ok(value) => Some(value),
        Err(what) => {
            problems.push(path.member(name), what);
            None
        }
    }
}
pub(crate) fn boolean(
    object: &mut read::Object,
    document: &Document,
    name: &[u8],
    path: &Path,
    problems: &mut Problems,
) -> Option<bool> {
    let at = object.required(document, name, path, problems)?;
    match read::boolean(document, at) {
        Ok(value) => Some(value),
        Err(what) => {
            problems.push(path.member(name), what);
            None
        }
    }
}
pub(crate) fn address(
    object: &mut read::Object,
    document: &Document,
    name: &[u8],
    path: &Path,
    problems: &mut Problems,
) -> Option<Address> {
    let at = object.required(document, name, path, problems)?;
    address::read(document, at, &path.member(name), problems)
}
pub(crate) fn list(
    object: &mut read::Object,
    document: &Document,
    name: &[u8],
    max: u32,
    path: &Path,
    problems: &mut Problems,
) -> Option<List<u32>> {
    let at = object.required(document, name, path, problems)?;
    match read::list(document, at, max) {
        Ok(value) => Some(value),
        Err(what) => {
            problems.push(path.member(name), what);
            None
        }
    }
}
pub(crate) fn local(address: &Address, path: &Path, problems: &mut Problems) {
    let local = match address {
        Address::Addr(Addr::Ip(socket)) => socket.ip().is_loopback(),
        Address::Addr(Addr::InProcess(_)) => true,
        Address::Host { .. } => false,
    };
    if !local {
        problems.push(path.clone(), What::Plaintext);
    }
}

/// The worker owns its hub and declares an end that fits the link and process.
pub(crate) fn worker(
    limits: &crate::Limits,
    graces: Option<crate::Graces>,
    problems: &mut Problems,
) -> Option<Duration> {
    let path = Path::root();
    let three_pings = limits.link.ping.as_nanos().checked_mul(3);
    let silence_fits = match three_pings {
        Some(minimum) => limits.link.silence.as_nanos() >= minimum,
        None => false,
    };
    if !silence_fits {
        problems.push(path.member(b"limits").member(b"link.silence_ms"), What::Silence);
    }
    let bound = match graces {
        Some(graces) => crate::stop_bound(&graces, limits.link.ping, limits.link.silence),
        None => None,
    };
    if graces.is_some() && bound.is_none() {
        problems.push(path.member(b"graces"), What::StopBound);
    }
    if !jig_domain_host::valid(&limits.hub) {
        problems.push(path.member(b"limits"), What::Bound);
    }
    bound
}
