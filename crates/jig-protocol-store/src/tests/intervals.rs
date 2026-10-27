//! Each range's intervals against its predicate (protocol/store.md, 4.3).
const PARTY: u64 = 1;
const TASK: u64 = 2;
use super::fixtures;
use crate::{intervals, keys};
use alloc::boxed::Box;
use jig_domain_core::{CoreKey, Key, RestoreRange};
use jig_domain_notes as notes;
use skein_lib::Rng;

fn key_bytes(key: &Key) -> Box<[u8]> {
    keys::encode(key, fixtures::KEY_LIMIT).expect("within the key limit")
}

// The existing generated sample count plus two explicit late-interval keys.
const SAMPLES: u32 = 500;
const BOUNDARY_KEYS: u32 = 2;
// A census and the combined core/tasks request, beside LIVE's single ranges.
const EXTRA_REQUESTS: u32 = 2;
fn samples() -> core::ops::Range<u32> {
    0..SAMPLES
}

fn cursor_requests() -> Box<[Box<[RestoreRange]>]> {
    // Six live ranges, a census, and a combined core/tasks request.
    let count = u32::try_from(RestoreRange::LIVE.len())
        .expect("fixed live count")
        .checked_add(EXTRA_REQUESTS)
        .expect("bounded requests");
    let mut requests = skein_lib::List::with_capacity(count);
    for range in RestoreRange::LIVE {
        requests.push(Box::from([range])).expect("counted requests");
    }
    requests.push(Box::from([RestoreRange::Census(census())])).expect("counted requests");
    requests.push(Box::from([RestoreRange::Core, RestoreRange::Tasks])).expect("counted requests");
    requests.into_boxed()
}

fn cursor_keys() -> Box<[Key]> {
    let mut rng = Rng::new(0x5701_e004);
    let mut keys = skein_lib::List::with_capacity(SAMPLES + BOUNDARY_KEYS);
    for _ in samples() {
        keys.push(fixtures::key(&mut rng)).expect("counted samples");
    }
    for key in [
        Key::People(jig_domain_people::Key::Acted(jig_domain_people::ActKey { connector: 1, key: [0; 16] })),
        Key::Tasks(jig_domain_tasks::Key::PlacedHold(TASK)),
    ] {
        keys.push(key).expect("counted boundary keys");
    }
    keys.into_boxed()
}

#[test]
#[expect(clippy::manual_map, reason = "the strict subset translates optional cursors without closures")]
fn each_restart_range_holds_the_keys_it_admits_and_no_others() {
    for key in cursor_keys() {
        let bytes = key_bytes(&key);
        for range in RestoreRange::LIVE {
            let inside = fixtures::within(&intervals::restore(&range), &bytes);
            assert_eq!(range.admits(&key), inside, "{range:?} and {key:?}");
        }
        for ranges in cursor_requests() {
            let walk = intervals::intervals(&ranges).expect("bounded restart ranges");
            let mut expected = None;
            for (index, interval) in walk.iter().enumerate() {
                if fixtures::within(core::slice::from_ref(interval), &bytes) {
                    expected = Some(crate::Cursor {
                        interval: u32::try_from(index).expect("bounded interval count"),
                        after: bytes.clone(),
                    });
                    break;
                }
            }
            assert_eq!(intervals::cursor(&ranges, &bytes), expected, "{ranges:?} and {key:?}");
            let typed = match jig_domain_core::restore_cursor(&ranges, &key) {
                Some((interval, _)) => Some(interval),
                None => None,
            };
            let encoded = match expected {
                Some(cursor) => Some(cursor.interval),
                None => None,
            };
            assert_eq!(typed, encoded, "typed and byte places agree: {ranges:?}, {key:?}");
        }
    }
}

#[test]
fn typed_and_encoded_cursors_order_the_same_restart_walk() {
    let keys = cursor_keys();
    let count = u32::try_from(keys.len()).expect("bounded keys");
    for ranges in cursor_requests() {
        let mut typed = skein_lib::List::with_capacity(count);
        let mut encoded = skein_lib::List::with_capacity(count);
        for key in &keys {
            if let Some(cursor) = jig_domain_core::restore_cursor(&ranges, key) {
                typed.push(cursor).expect("one cursor per key");
            }
            if let Some(cursor) = intervals::cursor(&ranges, &key_bytes(key)) {
                encoded.push((cursor.interval, cursor.after)).expect("one cursor per key");
            }
        }
        let mut typed = typed.into_boxed();
        let mut encoded = encoded.into_boxed();
        typed.sort();
        encoded.sort();
        let mut converted = skein_lib::List::with_capacity(count);
        for (interval, key) in typed {
            converted.push((interval, key_bytes(&key))).expect("one cursor per key");
        }
        assert_eq!(converted.into_boxed(), encoded, "walk order for {ranges:?}");
    }
}
#[test]
fn the_restart_reads_its_ranges_in_the_cores_order() {
    let expected = [&[1, 0, 0][..], &[1, 0, 8], &[1, 4, 4], &[1, 2, 0], &[1, 1, 0], &[1, 0, 1]];
    for (range, start) in RestoreRange::LIVE.into_iter().zip(expected) {
        let actual = intervals::restore(&range);
        assert_eq!(actual.first().expect("one live interval").start.as_ref(), start);
    }
}
#[test]
fn a_notes_load_holds_exactly_its_scopes_lines() {
    let mut rng = Rng::new(0x5701_e005);
    let wanted = notes::Scope::Project { project: 1 };
    let range = notes::Range::Lines { scope: wanted.clone(), after: Some(1) };
    let intervals = intervals::notes(&range);
    for _ in samples() {
        let key = fixtures::key(&mut rng);
        let admitted = match &key {
            Key::Notes(notes::Key::Line { scope, name }) => scope == &wanted && *name > 1,
            Key::Notes(notes::Key::Entry { .. }) | Key::Core(_) | Key::People(_) | Key::Tasks(_) | Key::Accounts(_) => {
                false
            }
        };
        let bytes = key_bytes(&key);
        assert_eq!(fixtures::within(&intervals, &bytes), admitted, "{key:?}");
    }
}
#[test]
fn a_notes_entry_load_holds_only_its_globally_named_entry() {
    let intervals = intervals::notes(&notes::Range::Entry { name: 1 });
    for key in [
        Key::Notes(notes::Key::Entry { name: 0 }),
        Key::Notes(notes::Key::Entry { name: 1 }),
        Key::Notes(notes::Key::Entry { name: 2 }),
        Key::Notes(notes::Key::Line { scope: notes::Scope::Deployment, name: 1 }),
    ] {
        let bytes = key_bytes(&key);
        let admitted = key == Key::Notes(notes::Key::Entry { name: 1 });
        assert_eq!(fixtures::within(&intervals, &bytes), admitted, "{key:?}");
    }
}
#[test]
fn no_restart_interval_holds_a_turn_terminal_or_note() {
    for key in [
        Key::Core(CoreKey::Turn { task: 1, attempt: 1, turn: 1 }),
        Key::Core(CoreKey::Terminal { task: 1, attempt: 1 }),
        Key::Notes(notes::Key::Entry { name: 1 }),
        Key::Notes(notes::Key::Line { scope: notes::Scope::Deployment, name: 1 }),
    ] {
        let bytes = key_bytes(&key);
        for range in RestoreRange::LIVE {
            assert!(!fixtures::within(&intervals::restore(&range), &bytes), "{range:?} excludes {key:?}");
        }
    }
}

#[test]
fn each_fixed_scope_holds_the_keys_it_admits_and_no_others() {
    use jig_domain_core::reads;
    let mut rng = Rng::new(0x5701_e006);
    let mut mappings = skein_lib::List::with_capacity(28);
    for scope in fixed_scopes() {
        for party in [1, 2] {
            let intervals = intervals::scope(party, &scope).expect("fixed scope");
            mappings.push((party, scope.clone(), intervals)).expect("fourteen scopes and two parties");
        }
    }
    for _ in samples() {
        let key = fixtures::key(&mut rng);
        let bytes = key_bytes(&key);
        for (party, scope, intervals) in &mappings {
            assert_eq!(
                reads::admits(*party, scope, &key),
                fixtures::within(intervals, &bytes),
                "{party}, {scope:?}, {key:?}"
            );
        }
    }
}

fn fixed_scopes() -> [jig_domain_core::reads::Scope; 14] {
    use jig_domain_core::reads::Scope;
    [
        Scope::Proposal { proposer: jig_domain_people::Proposer::Party(1), proposal: 1 },
        Scope::Proposal { proposer: jig_domain_people::Proposer::Party(2), proposal: 1 },
        Scope::SettingsHistory { first: 1, last: 3, newest: 3, whole: false },
        Scope::Spend { project: 1 },
        Scope::EndedDelegates { task: 1 },
        Scope::Task { task: 1 },
        Scope::PersonTask { task: 1 },
        Scope::Result { task: 1 },
        Scope::Reply { task: 1, activity: 1 },
        Scope::History { task: 1, subtree: false },
        Scope::Notes { project: 0 },
        Scope::Notes { project: 1 },
        Scope::ScopedNotes { scope: notes::Scope::Project { project: 1 }, description: None },
        Scope::ScopedNotes { scope: notes::Scope::Deployment, description: None },
    ]
}

fn inbox_scope(group: jig_domain_core::reads::Group) -> jig_domain_core::reads::Scope {
    use jig_domain_core::reads::{Filter, Scope};
    Scope::Inbox { group, filter: Filter { project: Some(1), goal: Some(1) } }
}

fn indexed_scopes() -> [jig_domain_core::reads::Scope; 3] {
    use jig_domain_core::reads::Group;
    [inbox_scope(Group::Updates), inbox_scope(Group::Results), inbox_scope(Group::Replies)]
}

fn indexed_mapping(
    party: u64,
    scope: jig_domain_core::reads::Scope,
) -> (u64, jig_domain_core::reads::Scope, Box<[skein_kv::Range]>) {
    let intervals = intervals::scope(party, &scope).expect("landed index family");
    (party, scope, intervals)
}

#[test]
fn each_indexed_scope_holds_the_keys_it_admits_and_no_others() {
    use jig_domain_core::reads;
    let mut rng = Rng::new(0x5701_e007);
    let mut mappings = skein_lib::List::with_capacity(22);
    for scope in indexed_scopes() {
        for party in [0, 1, 2, u64::MAX] {
            let mapping = indexed_mapping(party, scope.clone());
            mappings.push(mapping).expect("party, project and requester mappings fit");
        }
    }
    for requester in [0, 1, u64::MAX] {
        let mapping = indexed_mapping(1, reads::Scope::EndedDelegates { task: requester });
        mappings.push(mapping).expect("party, project and requester mappings fit");
    }
    for _ in 0_u32..1_000 {
        let key = fixtures::key(&mut rng);
        let bytes = key_bytes(&key);
        for (party, scope, intervals) in &mappings {
            assert_eq!(
                reads::admits(*party, scope, &key),
                fixtures::within(intervals, &bytes),
                "{party}, {scope:?}, {key:?}"
            );
        }
    }
}

#[test]
fn a_partys_updates_interval_holds_no_other_partys_rows() {
    use jig_domain_core::reads::Group;
    use jig_domain_tasks::{Key as TaskKey, Newest, UpdateObject};
    let ranges = intervals::scope(1, &inbox_scope(Group::Updates)).expect("updates index");
    for party in [0, 1, 2, u64::MAX] {
        for object in [UpdateObject::Result, UpdateObject::Reply { message: Newest::of(1), activity: Newest::of(1) }] {
            let key = Key::Tasks(TaskKey::Update { party, at: Newest::of(1), task: Newest::of(1), object });
            assert_eq!(fixtures::within(&ranges, &key_bytes(&key)), party == 1, "{key:?}");
        }
    }
}

fn census() -> jig_domain_core::Census {
    use jig_domain_core::{Census, ReadResults};
    Census {
        questions: Box::new([]),
        results: Box::new([
            ReadResults { party: 2, after: 1 },
            ReadResults { party: 1, after: 0 },
            ReadResults { party: u64::MAX, after: u64::MAX },
        ]),
    }
}

#[test]
fn the_census_holds_the_keys_it_admits_and_no_others() {
    use jig_domain_core::Census;
    let mut rng = Rng::new(0x5701_e008);
    let empty = RestoreRange::Census(Census { questions: Box::new([]), results: Box::new([]) });
    let listed = RestoreRange::Census(census());
    let mappings = [(&empty, intervals::restore(&empty)), (&listed, intervals::restore(&listed))];
    for _ in 0_u32..2_000 {
        let key = fixtures::key(&mut rng);
        let bytes = key_bytes(&key);
        for (range, intervals) in &mappings {
            assert_eq!(range.admits(&key), fixtures::within(intervals, &bytes), "{range:?}, {key:?}");
            if let RestoreRange::Census(census) = range
                && let Some(index) = census.interval(&key)
            {
                assert!(fixtures::within(
                    core::slice::from_ref(&intervals[usize::try_from(index).expect("bounded interval")]),
                    &bytes
                ));
            }
        }
    }
}

#[test]
fn the_census_reads_ended_read_positions_then_results_in_listed_order() {
    let ranges = intervals::restore(&RestoreRange::Census(census()));
    let starts: [&[u8]; 4] = [
        &[1, 2, 7],
        &[1, 1, 13, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 1, 0],
        &[1, 1, 13, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        &[1, 1, 13, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 0],
    ];
    assert_eq!(ranges.len(), starts.len());
    for (range, expected) in ranges.iter().zip(starts) {
        assert_eq!(range.start.as_ref(), expected);
    }
}

#[test]
fn a_requesters_delegates_interval_holds_its_edges_only() {
    use jig_domain_tasks::Key as TaskKey;
    let mut rng = Rng::new(0x5701_e009);
    let mappings = [
        (0, intervals::delegates(0)),
        (1, intervals::delegates(1)),
        (2, intervals::delegates(2)),
        (u64::MAX, intervals::delegates(u64::MAX)),
    ];
    for _ in samples() {
        let key = fixtures::key(&mut rng);
        let bytes = key_bytes(&key);
        for (requester, interval) in &mappings {
            let admitted = match &key {
                Key::Tasks(TaskKey::Delegate { requester: owner, .. }) => owner == requester,
                Key::Tasks(_) | Key::Core(_) | Key::People(_) | Key::Notes(_) | Key::Accounts(_) => false,
            };
            assert_eq!(admitted, fixtures::within(core::slice::from_ref(interval), &bytes), "{requester}, {key:?}");
        }
    }
}

#[test]
fn the_accounts_restore_range_stops_before_the_installations_row() {
    let ranges = intervals::restore(&RestoreRange::Accounts);
    let installation = keys::installation(fixtures::KEY_LIMIT).expect("within the key limit");
    assert!(!fixtures::within(&ranges, &installation), "tag 3 is no domain record");
    for key in [
        jig_domain_accounts::Key::Issued { number: u32::MAX },
        jig_domain_accounts::Key::Next,
        jig_domain_accounts::Key::Account { number: u32::MAX },
        jig_domain_accounts::Key::Source { account: u32::MAX, number: u32::MAX },
    ] {
        assert!(fixtures::within(&ranges, &key_bytes(&Key::Accounts(key))), "{key:?}");
    }
}

#[test]
fn the_people_restore_ranges_include_acted_and_skip_ended_read_positions() {
    let ranges = intervals::restore(&RestoreRange::People);
    for connector in [0, u16::MAX] {
        let key = Key::People(jig_domain_people::Key::Acted(jig_domain_people::ActKey { connector, key: [255; 16] }));
        assert!(fixtures::within(&ranges, &key_bytes(&key)), "{key:?}");
    }
    assert_eq!(ranges[0].end.as_deref(), Some([keys::JIG, 2, 7].as_slice()));
    assert!(!fixtures::within(
        &ranges,
        &key_bytes(&Key::People(jig_domain_people::Key::EndedRead { task: TASK, party: PARTY }))
    ));
}

#[test]
fn the_census_reads_only_the_named_question_activity_positions() {
    use jig_domain_core::Census;
    let census = Census { questions: Box::new([(2, 3), (u64::MAX, u64::MAX)]), results: Box::new([]) };
    let ranges = intervals::restore(&RestoreRange::Census(census.clone()));
    for task in [0, 1, 2, u64::MAX] {
        for position in [0, 2, 3, 4, u64::MAX] {
            let key = Key::Tasks(jig_domain_tasks::Key::Activity { task, position });
            let wanted = census.questions.contains(&(task, position));
            assert_eq!(census.admits(&key), wanted);
            assert_eq!(fixtures::within(&ranges, &key_bytes(&key)), wanted);
            if let Some(index) = census.interval(&key) {
                assert!(fixtures::within(
                    core::slice::from_ref(&ranges[usize::try_from(index).unwrap()]),
                    &key_bytes(&key)
                ));
            }
        }
    }
}

#[test]
fn tasks_restore_includes_placed_holds_beyond_the_history_families() {
    let ranges = intervals::restore(&RestoreRange::Tasks);
    for number in [0, 1, u64::MAX] {
        assert!(fixtures::within(&ranges, &key_bytes(&Key::Tasks(jig_domain_tasks::Key::PlacedHold(number)))));
        assert!(fixtures::within(&ranges, &key_bytes(&Key::Tasks(jig_domain_tasks::Key::Live(number)))));
        assert!(!fixtures::within(
            &ranges,
            &key_bytes(&Key::Tasks(jig_domain_tasks::Key::History { task: number, revision: 1 }))
        ));
    }
}
