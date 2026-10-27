//! Engine-only native world configuration.
// People's projects includes the configured project and the deployment's roles.
const ENGINE: &[u8] = br#"{
"limits": {"profile": "small", "overrides": {"tasks.result_bytes":32,"people.amendment_bytes":64,"core.policy_bytes":512,"tasks.spec_bytes":128,"tasks.message_bytes":512,"tasks.hold_bytes":64,"tasks.authority_bytes":64,"people.words":512,"core.call_answer_bytes":2048,"notes.recalled":1,"people.operations":2,"tasks.tasks":2,"tasks.exemptions":2,"tasks.funders":2,"tasks.project_tasks":1,"tasks.tree_tasks":1,"tasks.project_goals":1,"tasks.batch":1,"tasks.dependencies":1,"tasks.delegates":1,"tasks.charters":1,"people.people":1,"people.projects":2,"people.sign_ins":1,"people.holdings":1,"people.inbox_tasks":4,"people.pending":1,"people.waiters":1,"fleet.attempts":1,"fleet.calls":1,"fleet.turns":8,"brief.briefs":1,"brief.sections":1,"notes.entries_per_scope":1,"views.runs":1,"views.watchers":2,"views.watchers_per_party":2,"core.load_slots":1,"core.load_bytes":8192,"core.resume_bytes":4096,"core.run_bytes":4096,"views.text_bytes":1024,"core.templates":2,"documents.rows":1,"documents.lines":1,"documents.chips":1,"documents.text_bytes":128}},
"llm": {
"providers":[{"name":"Fake","number":1,"models":["small"],"sources":{}}],
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
{"number": 1, "name": "chat", "executor": {"charter": 1}, "contract": {"report": {"words": 32}}, "shallower": 1, "budget": {"as_allowed": null}, "deadline": "none", "tracked": null, "conversation": true, "listed": false, "retention": "kept", "label": 1, "grace_ms": 5000, "llm": null},
{"number": 2, "name": "goal", "executor": "chosen", "contract": {"report": {"words": 32}}, "shallower": 1, "budget": "given", "deadline": "given", "tracked": {"priority": 1}, "conversation": false, "listed": true, "retention": "kept", "label": 2, "grace_ms": null, "llm": null}
],
"clients": {
"heartbeat_ms": 10000,
"link": true
},
"deployment": {"directory": "state", "seed": 7},
"owners": [{"provider": 1, "subject": "owner-a"}],
"providers": [{"number": 1, "kind": "link"}, {"number": 2, "kind": "deployment"}],
"connectors": [1,2],
"memory_bytes": 1152921504606846976,
"rules": {
"ceiling": {"tools": 1023, "grants": [],
"delegation": {"kinds": [{"kind": "charter", "number": 1}], "tasks": 8, "depth": 8},
"budget": {"spend": 100, "deadline_unix_seconds": null}, "notes": 7, "note_resources": []},
"period_spend": 100, "minimum_run_spend": 1, "maximum_run_spend": 100,
"implies": [], "requirements": []
},
"projects": [{"number": 1, "roles": [{"connector": 1, "permission": 1, "role": 0}],
"policy": {"templates": [], "escalation_role": 0, "period_spend": 100, "projections": [], "requirements": [],
"ceiling": {"tools": 1023, "grants": [],
"delegation": {"kinds": [{"kind": "charter", "number": 1}], "tasks": 8, "depth": 8},
"budget": {"spend": 100, "deadline_unix_seconds": null}, "notes": 7, "note_resources": []},
"roles": [{"number": 0, "period_spend": 100, "requests": 511, "decides": 31,
"authority": {"tools": 1023, "grants": [],
"delegation": {"kinds": [{"kind": "charter", "number": 1}], "tasks": 8, "depth": 8},
"budget": {"spend": 100, "deadline_unix_seconds": null}, "notes": 7, "note_resources": []}}]
}}]
}"#;
pub(super) fn document() -> Vec<u8> {
    let mut bytes = b"{\"engine\":".to_vec();
    bytes.extend_from_slice(ENGINE);
    bytes.extend_from_slice(br#", "application":{"server":{"listener":{"socket":"127.0.0.1:7100"}},"credential":"app.secret","connectors":[{"prefix":["one"],"resources":[]},{"prefix":["two"],"resources":[]}]},"service":{"components":["clients"]}}"#);
    bytes
}
