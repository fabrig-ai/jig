//! The application's limits and channel edges (configuration.md, 6).
use jig_protocol_config::{Profile, profile};
use jig_test_connector as connector;
use jig_test_domain as root;
use skein_lib::Duration;

pub const SMALL: connector::Limits = connector::Limits {
    tasks: 8,
    adoptions: 16,
    subscriptions: 16,
    pools: 4,
    resources: 16,
    topics: 4,
    kinds: 8,
    requirements: 4,
    procedures: 4,
    actions_per_procedure: 8,
    facts: 16,
    judges: 4,
    values: 8,
    value_bytes: 4096,
    staged: 8,
    entries: 8,
    made: 16,
    resources_per_effect: 4,
    max_attempts: 3,
    write_lifetime: Duration::from_secs(30),
    clock_margin: Duration::from_secs(5),
    resources_per_task: 4,
    subscribers_per_topic: 8,
    lost_per_pool: 8,
    path_segments: 4,
    segment_bytes: 256,
};
pub const STANDARD: connector::Limits = connector::Limits {
    tasks: 32,
    adoptions: 64,
    subscriptions: 64,
    resources: 64,
    facts: 64,
    values: 32,
    staged: 32,
    entries: 32,
    made: 64,
    ..SMALL
};

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub root: root::Limits,
    pub store: jig_protocol_store::Limits,
    pub io: skein_io::Limits,
    pub accounts: skein_oauth_accounts::Limits,
    pub workers: jig_protocol_engine_workers::Limits,
    pub clients: jig_protocol_engine_clients::Limits,
    pub clients_http: jig_protocol_engine_clients_http::Limits,
    pub server: crate::server::Limits,
}
#[must_use]
pub fn for_profile(name: Profile, jig: &jig_protocol_config::Limits) -> Limits {
    let connector = match name {
        Profile::Small => SMALL,
        Profile::Standard => STANDARD,
    };
    let accounts = jig.driver;
    Limits {
        root: root::Limits {
            core: jig.core,
            connector,
            journal: jig.journal,
            watermark: jig.journal.commits,
            routes: jig.routes,
        },
        store: jig_protocol_store::Limits { core: jig.core, journal: jig.journal, ..jig.store },
        io: jig.io,
        accounts,
        workers: workers(jig),
        clients: jig.clients,
        clients_http: jig.clients_http,
        server: crate::server::Limits {
            connections: match name {
                Profile::Small => 8,
                Profile::Standard => 16,
            },
            http: skein_http::server::Limits {
                head: 4096,
                headers: 32,
                body: u64::from(jig.clients_http.body),
                read: jig.clients_http.read,
                response: 1024,
                send: jig.clients_http.send,
            },
            stall: Duration::from_secs(30),
        },
    }
}
#[must_use]
pub fn profile_limits(name: Profile) -> Limits {
    for_profile(name, &profile(name))
}

fn workers(jig: &jig_protocol_config::Limits) -> jig_protocol_engine_workers::Limits {
    let core = jig.core;
    let hub = jig.hub;
    let link = jig.link;
    let ceilings = jig_protocol_worker_wire::CEILINGS;
    let sizes = jig_protocol_worker_wire::sizes::Sizes {
        program_sources: core.fleet.program_sources,
        piece: hub.piece_bytes,
        charter: u32::try_from(hub.charter_bytes).expect("channel byte bound"),
        transcript: u32::try_from(hub.transcript_bytes).expect("channel byte bound"),
        turn: u32::try_from(hub.turn_bytes).expect("channel byte bound"),
        message: core.tasks.message_bytes,
        sender: ceilings.message_sender,
        call_name: u32::try_from(core.fleet.call_name_bytes).expect("call name bound"),
        tool_name: ceilings.call_tool,
        call_input: core.call_answer_bytes,
        answer: core.call_answer_bytes,
        result: u32::try_from(hub.outcome_bytes).expect("channel byte bound"),
        detail: hub.detail_bytes,
        grant: jig.driver.client.document.token_bytes,
        name: link.name,
        secret: link.secret,
        model_name: ceilings.model_bound_name,
        items: 0,
        left: 0,
        saved: 0,
        evidence: u32::try_from(hub.delivery_evidence_bytes).expect("channel byte bound"),
        resource: 0,
        slots: core.fleet.slots,
        calls: hub.run_calls,
        turns: hub.turns,
        inbox: core.tasks.run_inbox,
        answered: core.call_records,
        models: ceilings.bounds_models,
        workstreams: core.fleet.workstreams,
        accounts: core.accounts.accounts,
        credentials: core.accounts.credentials,
        facts: link.facts,
    };
    jig_protocol_engine_workers::Limits {
        sizes,
        names: link.names,
        workers: core.fleet.workers,
        opening: link.opening,
        chunk: link.chunk,
        handshake: link.handshake,
        hello: link.hello,
        ping: link.ping,
        silence: link.silence,
        stall: link.stall,
    }
}

pub(super) fn worker_link(worker: &jig_protocol_config::Worker) -> jig_protocol_worker_link::Limits {
    let profile = worker.limits.link;
    let mut sizes = workers(&worker.limits).sizes;
    sizes.slots = worker.slots;
    jig_protocol_worker_link::Limits {
        sizes,
        chunk: profile.chunk,
        handshake: profile.handshake,
        ping: profile.ping,
        silence: profile.silence,
        stall: profile.stall,
        backoff: profile.backoff,
        ceiling: profile.ceiling,
        proved: profile.proved,
        grace: worker.graces.grace,
        busy: profile.busy,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_profile_reserves_a_largest_piece_fact_and_maps_the_hubs_bound() {
        use jig_protocol_worker_wire::{Kind, sizes};
        for name in [Profile::Small, Profile::Standard] {
            let profile = profile(name);
            let sizes = workers(&profile).sizes;
            assert_eq!(sizes.piece, profile.hub.piece_bytes);
            let codec = sizes::limits(&sizes).expect("profile codec limits");
            assert_eq!(codec.piece_bytes, profile.hub.piece_bytes);
            let frame = sizes::largest_body(&sizes, Kind::Fact).expect("fact body") + 8;
            assert_eq!(frame, 4470);
            let cap = sizes::worker_cap(&sizes).expect("worker cap");
            assert!(cap.facts >= frame, "{name:?}: reserve {} must hold {frame}", cap.facts);
            assert!(cap.cap.frames >= cap.facts / 25);
        }
    }
}
