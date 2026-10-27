use crate::{DocumentLimits, Path};
use skein_json::Document;
pub const LIMITS: DocumentLimits = DocumentLimits { bytes: 256, tokens: 128, depth: 8 };
pub const DIGEST_EMPTY: &[u8] = b"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855\n";
pub const SECRET_A: &[u8] = b"secret-a\n";
pub fn document(bytes: &[u8]) -> Document {
    crate::document(bytes, &LIMITS).expect("fixture JSON")
}
pub fn path(segments: &[&[u8]]) -> Path {
    let mut path = Path::root();
    for segment in segments {
        path = path.member(segment);
    }
    path
}

pub const ENGINE_SMALL: &[u8] = br#"{
"limits": {"profile": "small"},
"llm": {
"providers": [{"name":"Fake","number":1,"models":["small"],"sources":{}}],
"endpoints": [{
"name": "fake",
"number": 1,
"dialect": 0,
"provider": "fake-provider",
"address": {"socket": "127.0.0.1:7101"},
"transport": "plaintext",
"headers": [],
"connect_ms": 10000,
"handshake_ms": 10000
}],
"models": [{
"name": "small",
"endpoint": "fake",
"window": 8192,
"output": 1024,
"prices": {"input": 1, "cached": 1, "output": 1, "unit": 1000}
}]
},
"hosts": {
"workers": [
{"name": "worker-a", "digest": "secrets/worker-a.digest"},
{"name": "worker-b", "digest": "secrets/worker-b.digest"}
],
"listener": [{"in_process": "workers"}]
},
"charters": [{
"number": 1, "instructions": "charters/chat.txt", "model": "small", "alternatives": [],
"hosts": "workers", "waiting_ms": 10000, "resume": true,
"turns": 4, "time_ms": 30000, "reserve_turns": 1, "reserve_time_ms": 5000, "spend": 20,
"inspect": true, "modify": false, "shell": false, "agents": false, "tools": 2047, "call_timeout_ms": 10000
}],
"templates": [
{"number": 1, "name": "chat", "executor": {"charter": 1}, "contract": {"report": {"words": 512}}, "shallower": 1, "budget": {"as_allowed": null}, "deadline": "none", "tracked": null, "conversation": true, "listed": false, "retention": "kept", "label": 1, "grace_ms": 5000, "llm": null},
{"number": 2, "name": "goal", "executor": "chosen", "contract": {"report": {"words": 512}}, "shallower": 1, "budget": "given", "deadline": "given", "tracked": {"priority": 1}, "conversation": false, "listed": true, "retention": "kept", "label": 2, "grace_ms": null, "llm": null}
],
"clients": {
"heartbeat_ms": 10000,
"link": true
},
"deployment": {"directory": "state", "seed": 7},
"owners": [{"provider": 1, "subject": "owner-a"}],
"providers": [{"number": 1, "kind": "link"}, {"number": 2, "kind": "deployment"}],
"connectors": [1],
"memory_bytes": 34359738368,
"rules": {
"ceiling": {"tools": 1023, "grants": [],
"delegation": {"kinds": [{"kind": "charter", "number": 1}], "tasks": 8, "depth": 8},
"budget": {"spend": 100, "deadline_unix_seconds": null}, "notes": 7, "note_resources": []},
"period_spend": 100, "minimum_run_spend": 1, "maximum_run_spend": 100,
"implies": [], "requirements": []
},
"projects": [{"number": 1, "roles": [{"connector": 1, "permission": 1, "role": 1}],
"policy": {"templates": [], "escalation_role": 1, "period_spend": 100, "projections": [], "requirements": [],
"ceiling": {"tools": 1023, "grants": [],
"delegation": {"kinds": [{"kind": "charter", "number": 1}], "tasks": 8, "depth": 8},
"budget": {"spend": 100, "deadline_unix_seconds": null}, "notes": 7, "note_resources": []},
"roles": [{"number": 1, "period_spend": 100, "requests": 511, "decides": 31,
"authority": {"tools": 1023, "grants": [],
"delegation": {"kinds": [{"kind": "charter", "number": 1}], "tasks": 8, "depth": 8},
"budget": {"spend": 100, "deadline_unix_seconds": null}, "notes": 7, "note_resources": []}}]
}}]
}"#;
pub fn engine(bytes: &[u8]) -> Result<crate::Engine, crate::Problems> {
    let limits = DocumentLimits { bytes: 8192, tokens: 1536, depth: 16 };
    let document = match crate::document(bytes, &limits) {
        Ok(document) => document,
        Err(problem) => {
            let mut problems = crate::Problems::empty();
            problems.push(problem.path, problem.what);
            return Err(problems);
        }
    };
    crate::engine(&document, 0, &crate::Directories::working(), crate::Defaults::Document)
}
pub fn with(document: &[u8], from: &[u8], to: &[u8]) -> Box<[u8]> {
    let at = skein_lib::bytes::find(document, from).expect("fixture replacement exists");
    let end = at.checked_add(from.len()).expect("fixture fits usize");
    let len = document.len().checked_sub(from.len()).unwrap().checked_add(to.len()).unwrap();
    let mut writer = skein_lib::Writer::new(len);
    writer.put(document.get(..at).unwrap()).unwrap();
    writer.put(to).unwrap();
    writer.put(document.get(end..).unwrap()).unwrap();
    writer.finish()
}
pub fn refuses(from: &[u8], to: &[u8], path: Path, what: crate::What) {
    let bytes = with(ENGINE_SMALL, from, to);
    let problems = engine(&bytes).unwrap_err();
    assert_eq!(problems.problems.get(0), Some(&crate::Problem { path, what }));
}
pub const WORKER_SMALL: &[u8] = br#"{
"engine": {"in_process": "workers"},
"name": "worker-a", "secret": "secrets/worker-a.secret", "slots": 2,
"programs":{"check_interval_ms":60000},
"graces": {"grace_ms": 30000, "agent_stop_ms": 10000, "delivery_ms": 20000, "save_ms": 5000},
"agent": {"program": "jig-test", "arguments": ["agent"], "configuration": "agent.json"},
"limits": {"profile": "small"}, "memory_bytes": 1073741824
}"#;

/// `ENGINE_SMALL` ends with its projects array, which this fixture empties whole.
pub fn without_projects(document: &[u8]) -> Box<[u8]> {
    let prefix = br#""projects": ["#;
    let start = skein_lib::bytes::find(document, prefix).unwrap().checked_add(prefix.len()).unwrap();
    let end = document.len().checked_sub(3).unwrap();
    with(document, &document[start..end], b"")
}
