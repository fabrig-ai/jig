//! Shipped profiles fit the byte-summed families (store.md, 5.1).
use jig_protocol_config::{Profile, profile};
use jig_protocol_store::codec;

#[test]
fn both_shipped_profiles_admit_the_tasks_and_people_families() {
    for profile_name in [Profile::Small, Profile::Standard] {
        let limits = profile(profile_name);
        assert!(codec::tasks::limits(&limits.core.tasks).is_some(), "{profile_name:?} Tasks");
        assert!(codec::people::limits(&limits.core).is_some(), "{profile_name:?} People");
    }
}

#[test]
fn both_shipped_profiles_round_trip_the_settled_call_sum() {
    use jig_domain_core as core;
    // The shipped call-answer bound is shared by the tool and the host body.
    const ANSWER_BYTES: usize = 32_768;
    const HALF_ANSWER: usize = ANSWER_BYTES / 2;
    let answers = [
        (
            "host",
            core::SettledAnswer::Host { error: false, body: vec![1; HALF_ANSWER].into_boxed_slice() },
            HALF_ANSWER,
        ),
        ("delivery", core::SettledAnswer::Delivery { outcome: core::DeliveryOutcome::Delivered }, ANSWER_BYTES),
    ];
    for profile_name in [Profile::Small, Profile::Standard] {
        let domain = profile(profile_name);
        let limits = codec::limits(&domain.core).expect("shipped profile within codec ceilings");
        assert_eq!(domain.core.call_answer_bytes, 32_768, "{profile_name:?}");
        let core_limits = codec::core::Limits { core: limits.core, tasks: limits.tasks, notes: limits.notes };
        for (label, answer, tool_bytes) in &answers {
            let row = core::CoreRecord::Call(core::CallRecord {
                key: core::CallKey { task: 1, attempt: 2, completion: 3, position: 4 },
                part: core::CallPart::Unavailable,
                settled: Some(core::SettledCall {
                    serial: 1,
                    tool: vec![3; *tool_bytes].into_boxed_slice(),
                    answer: answer.clone(),
                }),
            });
            let bytes = codec::core::encode(&row, &core_limits).expect("sum at its limit");
            assert_eq!(codec::core::decode(&bytes, &core_limits), Ok(row), "{profile_name:?} {label}");
        }
    }
}

const STORE: skein_kv::Limits = skein_kv::Limits {
    key: 2_079,
    value: 268_435_456,
    ops: 1_000_000,
    commit: 1_073_741_824,
    queued: 3,
    queued_bytes: 3_221_225_472,
    budget: 268_435_456,
    segment: 1_073_741_824,
    snapshot_after: 268_435_456,
    chunk: 268_437_555,
    page: skein_kv::Page { rows: 2, bytes: 268_437_535 },
    deadline: skein_lib::Duration::from_nanos(1_000_000_000),
};

#[test]
fn both_shipped_profiles_are_admitted_by_the_store() {
    let application = jig_protocol_store::sizes::Application { version: 1, key: 2_078, value: 4_304 };
    assert!(STORE.is_usable());
    for profile_name in [Profile::Small, Profile::Standard] {
        let limits = profile(profile_name);
        assert!(codec::limits(&limits.core).is_some(), "{profile_name:?}");
        let sizes = jig_protocol_store::sizes::check(&limits.core, &application, &limits.journal, &STORE, 1)
            .expect("shipped profile fits the map-budget commit envelope");
        println!(
            "{profile_name:?}: largest key={}, value={}, commit={}, ops={}",
            sizes.key, sizes.value, sizes.commit, sizes.ops
        );
    }
}

#[test]
fn both_shipped_profiles_round_trip_a_policy_whose_roles_name_models() {
    use jig_domain_people as people;
    // As many models as a role's bits name, each name as long as configuration admits.
    let models = u8::try_from(u64::BITS).expect("one bit per model");
    let name_bytes = usize::try_from(jig_domain_core::MODEL_NAME_BYTES).expect("name bound");
    let names: Box<[Box<[u8]>]> = (0..models).map(|model| vec![model; name_bytes].into_boxed_slice()).collect();
    let role = |number, models| people::PolicyRole {
        number,
        authority: people::Authority {
            tools: 1,
            models,
            grants: Box::new([]),
            delegation: people::Delegation { kinds: Box::new([]), tasks: 0, depth: 0 },
            spend: 0,
            deadline: None,
            notes: 0,
            note_resources: Box::new([]),
        },
        period_spend: 0,
        requests: 0,
        decides: 0,
    };
    let row = people::Stored::Policy {
        name: Box::from("Tally"),
        project: 1,
        value: people::PolicyValue {
            ceiling: role(0, Some(people::Models::ALL)).authority,
            escalation_role: Some(0),
            period_spend: 0,
            projections: Box::new([]),
            roles: Box::new([
                role(0, Some(people::Models::ALL)),
                role(1, Some(people::Models::Named((1 << 63) | 0b101))),
            ]),
            requirements: Box::new([]),
            permissions: Box::new([]),
            templates: Box::new([]),
        },
    };
    for profile_name in [Profile::Small, Profile::Standard] {
        let limits = codec::people::limits(&profile(profile_name).core).expect("shipped profile within ceilings");
        let bytes = codec::people::encode(&row, &limits, &names).expect("every name within people's bounds");
        assert_eq!(codec::people::decode(&bytes, &limits, &names), Ok(row.clone()), "{profile_name:?}");
    }
}
