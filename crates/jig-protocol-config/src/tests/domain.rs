use crate::{What, tests::fixtures};

fn engine() -> crate::Engine {
    fixtures::engine(fixtures::ENGINE_SMALL).expect("whole engine fixture")
}
pub(super) fn core_config(document: &[u8]) -> jig_domain_core::Config {
    let mut engine = fixtures::engine(document).expect("checked engine");
    engine.charters[0].policy.instructions = Box::from(&b"Answer the question."[..]);
    crate::core(&engine, [3; 16], 11)
}
fn refuses(from: &[u8], to: &[u8], what: What) {
    let document = fixtures::with(fixtures::ENGINE_SMALL, from, to);
    let problems = fixtures::engine(&document).expect_err("refused domain configuration");
    assert_eq!(problems.problems.get(0).expect("one problem").what, what);
}
pub(super) fn two_projects(number: u8) -> Box<[u8]> {
    let prefix = br#""projects": ["#;
    let start = skein_lib::bytes::find(fixtures::ENGINE_SMALL, prefix).unwrap().checked_add(prefix.len()).unwrap();
    let end = fixtures::ENGINE_SMALL.len().checked_sub(3).unwrap();
    let project = fixtures::ENGINE_SMALL.get(start..end).unwrap();
    let second = fixtures::with(
        project,
        br#""number": 1"#,
        &[b'"', b'n', b'u', b'm', b'b', b'e', b'r', b'"', b':', b' ', number],
    );
    let mut writer = skein_lib::Writer::new(project.len().checked_add(second.len()).unwrap().checked_add(1).unwrap());
    writer.put(project).unwrap();
    writer.put(b",").unwrap();
    writer.put(&second).unwrap();
    fixtures::with(fixtures::ENGINE_SMALL, project, &writer.finish())
}
const CONFIGURED_PROJECTS: u32 = 2; // two_projects seeds projects 1 and 2.
const DEPLOYMENT_ROSTER: u32 = 1;
const ALL_ROSTERS: u32 = CONFIGURED_PROJECTS + DEPLOYMENT_ROSTER;

fn two_ownerless_projects(people_projects: u32) -> Box<[u8]> {
    let document = fixtures::with(
        &two_projects(b'2'),
        br#""owners": [{"provider": 1, "subject": "owner-a"}]"#,
        br#""owners": []"#,
    );
    let limits = fixtures::with(
        br#""profile": "small", "overrides": {"people.projects": CAP}"#,
        b"CAP",
        skein_lib::Decimal::of(u64::from(people_projects)).as_bytes(),
    );
    fixtures::with(&document, br#""profile": "small""#, &limits)
}
#[test]
fn configured_projects_above_the_overridden_people_capacity_are_refused() {
    let problems = fixtures::engine(&two_ownerless_projects(ALL_ROSTERS - DEPLOYMENT_ROSTER))
        .expect_err("three rosters exceed capacity two");
    assert_eq!(
        problems.problems.as_slice(),
        &[crate::Problem { path: crate::Path::root().member(b"projects"), what: What::Bound }]
    );
}
#[test]
fn configured_projects_at_or_below_the_overridden_people_capacity_parse() {
    for capacity in [ALL_ROSTERS, ALL_ROSTERS + DEPLOYMENT_ROSTER] {
        let engine =
            fixtures::engine(&two_ownerless_projects(capacity)).expect("two projects and the deployment roster fit");
        assert_eq!(engine.limits.core.people.projects, capacity);
        assert_eq!(engine.projects.len(), 2);
        assert_eq!([engine.projects[0].number, engine.projects[1].number], [1, 2]);
        assert!(engine.owners.is_empty(), "ownerless projects still need rosters");
    }
}
#[test]
fn a_configured_project_numbered_zero_is_refused() {
    let document =
        fixtures::with(fixtures::ENGINE_SMALL, br#""projects": [{"number": 1"#, br#""projects": [{"number": 0"#);
    let problems = fixtures::engine(&document).expect_err("zero is the deployment roster");
    assert_eq!(
        problems.problems.as_slice(),
        &[crate::Problem {
            path: fixtures::path(&[b"projects"]).child(crate::Segment::Index(0)).member(b"number"),
            what: What::Bound,
        }]
    );
}

fn with_grants(connector: u8) -> Box<[u8]> {
    let grant = br#""grants": [{"connector": 1, "kind": 2, "pattern": {"segments": ["service"], "last": "open", "value": ""}}]"#;
    let grant = fixtures::with(
        grant,
        br#""connector": 1"#,
        &[b'"', b'c', b'o', b'n', b'n', b'e', b'c', b't', b'o', b'r', b'"', b':', b' ', connector],
    );
    let mut document = fixtures::with(fixtures::ENGINE_SMALL, br#""grants": []"#, &grant);
    for _ in 0_u32..2 {
        document = fixtures::with(&document, br#""grants": []"#, &grant);
    }
    document
}
#[test]
fn the_small_engine_document_parses_whole() {
    let engine = engine();
    assert_eq!(engine.deployment.directory.as_ref(), b"state");
    assert_eq!(engine.deployment.seed, Some(7));
    assert_eq!(engine.providers.len(), 2);
}

#[test]
fn operations_below_delegates_plus_one_refuse_the_start() {
    refuses(
        br#""profile": "small""#,
        br#""profile": "small", "overrides": {"people.operations": 4, "tasks.project_goals": 4}"#,
        What::Operations { operations: 4, delegates: 4, goals: 4 },
    );
}

#[test]
fn operations_below_goals_per_project_refuse_the_start() {
    refuses(
        br#""profile": "small""#,
        br#""profile": "small", "overrides": {"people.operations": 7}"#,
        What::Operations { operations: 7, delegates: 4, goals: 8 },
    );
}

#[test]
fn the_shipped_profiles_hold_operations_per_request() {
    for (profile, expected) in [(crate::Profile::Small, (8, 4, 8)), (crate::Profile::Standard, (16, 8, 16))] {
        let limits = crate::profile(profile).core;
        assert_eq!((limits.people.operations, limits.tasks.delegates, limits.tasks.project_goals), expected);
        assert!(limits.people.operations > limits.tasks.delegates, "{profile:?}");
        assert!(limits.people.operations >= limits.tasks.project_goals, "{profile:?}");
    }
}
#[test]
fn the_cores_config_takes_the_charters_and_projects() {
    let core = core_config(fixtures::ENGINE_SMALL);
    assert_eq!(core.defaults.charters.len(), 1);
    assert_eq!(core.seed_projects.len(), 1);
    assert_eq!(core.seed_projects[0].number, 1);
    assert_eq!(core.defaults.charters[0].policy.instructions.as_ref(), b"Answer the question.");
}
#[test]
fn the_cores_policy_and_permissions_are_seeded_per_project() {
    let core = core_config(&two_projects(b'2'));
    assert_eq!(core.seed_projects[1].policy.policy.period_spend, 100);
    assert_eq!(
        core.seed_projects[1].policy.permissions.as_ref(),
        &[jig_domain_people::PermissionRole { connector: 1, permission: 1, role: 1 }]
    );
}
#[test]
fn the_cores_settings_take_the_deployment_provider() {
    let core = core_config(fixtures::ENGINE_SMALL);
    assert_eq!(core.settings.deployment_provider, 2);
}
#[test]
fn the_cores_limits_and_tools_take_the_profile_and_jigs_numbering() {
    let core = core_config(fixtures::ENGINE_SMALL);
    assert_eq!(core.settings.resume_bytes, 65536);
    assert_eq!(core.settings.tail_bytes, 2048);
    assert_eq!(core.hub_turns, 8);
    assert_eq!(core.settings.tools.message, jig_domain_authority::Tools(2));
}
#[test]
fn an_in_process_listener_tells_the_core_workers_share_its_process() {
    assert!(core_config(fixtures::ENGINE_SMALL).workers_in_process);
}
#[test]
fn a_socket_listener_tells_the_core_workers_have_their_own_process() {
    let document =
        fixtures::with(fixtures::ENGINE_SMALL, br#"{"in_process": "workers"}"#, br#"{"socket":"127.0.0.1:7102"}"#);
    assert!(!core_config(&document).workers_in_process);
}
#[test]
fn a_configured_seed_overrides_the_shells_draw() {
    assert_eq!(core_config(fixtures::ENGINE_SMALL).seed, 7);
}
#[test]
fn a_shell_seed_is_used_when_configuration_has_no_seed() {
    let document = fixtures::with(fixtures::ENGINE_SMALL, br#", "seed": 7"#, b"");
    assert_eq!(core_config(&document).seed, 11);
}
#[test]
fn a_project_numbered_twice_is_refused() {
    let problems = fixtures::engine(&two_projects(b'1')).expect_err("duplicate project");
    assert_eq!(problems.problems.get(0).unwrap().what, What::Unique);
}
fn an_owner_naming_an_unknown_provider_is_refused() {
    refuses(br#""provider": 1"#, br#""provider": 3"#, What::Refers);
}
fn a_message_bound_below_a_partys_words_is_refused() {
    refuses(br#""profile": "small""#, br#""profile": "small", "overrides": {"people.words": 4097}"#, What::Message);
}
fn a_retention_member_is_refused_as_unknown() {
    refuses(br#""directory": "state""#, br#""directory": "state", "retention": {}"#, What::Unknown);
}
fn a_projects_resources_member_is_refused_as_unknown() {
    refuses(br#""projects": [{"number": 1"#, br#""projects": [{"number": 1, "resources": []"#, What::Unknown);
}
fn a_projects_homes_member_is_refused_as_unknown() {
    refuses(br#""projects": [{"number": 1"#, br#""projects": [{"number": 1, "homes": []"#, What::Unknown);
}
fn two_deployment_providers_are_refused() {
    refuses(br#""kind": "link""#, br#""kind": "deployment""#, What::Unique);
}
fn a_deployment_provider_is_required() {
    refuses(br#""kind": "deployment""#, br#""kind": "application""#, What::Missing);
}
fn a_connector_numbered_as_the_cores_own_is_refused() {
    refuses(br#""connectors": [1]"#, br#""connectors": [65535]"#, What::Bound);
}
#[test]
fn a_ceiling_without_models_takes_every_configured_model() {
    let engine = engine();
    assert_eq!(engine.rules.ceiling.models, jig_domain_authority::Models::ALL);
    assert_eq!(engine.projects[0].policy.ceiling.models, jig_domain_authority::Models::ALL);
}
#[test]
fn a_ceilings_models_name_the_configured_models_it_keeps() {
    let ceiling = br#""ceiling": {"tools": 1023,"#;
    let named = fixtures::engine(&fixtures::with(
        fixtures::ENGINE_SMALL,
        ceiling,
        br#""ceiling": {"tools": 1023, "models": ["small"],"#,
    ));
    assert_eq!(named.unwrap().rules.ceiling.models, jig_domain_authority::Models::Named(0b1));
    let none = fixtures::engine(&fixtures::with(
        fixtures::ENGINE_SMALL,
        ceiling,
        br#""ceiling": {"tools": 1023, "models": [],"#,
    ));
    assert_eq!(none.unwrap().rules.ceiling.models, jig_domain_authority::Models::NONE);
}
fn a_ceiling_naming_an_unknown_model_is_refused() {
    fixtures::refuses(
        br#""ceiling": {"tools": 1023,"#,
        br#""ceiling": {"tools": 1023, "models": ["large"],"#,
        fixtures::path(&[b"rules", b"ceiling", b"models"]).child(crate::Segment::Index(0)),
        What::Refers,
    );
}
fn a_ceiling_naming_a_model_twice_is_refused() {
    fixtures::refuses(
        br#""ceiling": {"tools": 1023,"#,
        br#""ceiling": {"tools": 1023, "models": ["small", "small"],"#,
        fixtures::path(&[b"rules", b"ceiling", b"models"]).child(crate::Segment::Index(1)),
        What::Unique,
    );
}
const ROLE: &[u8] = br#""authority": {"tools": 1023,"#;
const NAMED: &[u8] = br#""authority": {"tools": 1023, "models": "#;
fn role_models(models: &[u8]) -> Result<crate::Engine, crate::Problems> {
    let mut writer = skein_lib::Writer::new(NAMED.len().checked_add(models.len()).unwrap().checked_add(1).unwrap());
    writer.put(NAMED).unwrap();
    writer.put(models).unwrap();
    writer.put(b",").unwrap();
    fixtures::engine(&fixtures::with(fixtures::ENGINE_SMALL, ROLE, &writer.finish()))
}
fn role(engine: &crate::Engine) -> jig_domain_authority::Models {
    engine.projects[0].policy.roles[0].authority.models
}
fn role_models_path(index: usize) -> crate::Path {
    fixtures::path(&[b"projects"])
        .child(crate::Segment::Index(0))
        .member(b"policy")
        .member(b"roles")
        .child(crate::Segment::Index(0))
        .member(b"authority")
        .member(b"models")
        .child(crate::Segment::Index(u32::try_from(index).unwrap()))
}
#[test]
fn a_roles_models_name_the_configured_models_it_keeps() {
    assert_eq!(role(&engine()), jig_domain_authority::Models::ALL, "absent names every configured model");
    assert_eq!(role(&role_models(br#"["small"]"#).unwrap()), jig_domain_authority::Models::Named(0b1));
    assert_eq!(role(&role_models(b"[]").unwrap()), jig_domain_authority::Models::NONE);
}
fn a_role_naming_an_unknown_model_is_refused() {
    let problems = role_models(br#"["large"]"#).unwrap_err();
    assert_eq!(problems.problems.get(0), Some(&crate::Problem { path: role_models_path(0), what: What::Refers }));
}
fn a_role_naming_a_model_twice_is_refused() {
    let problems = role_models(br#"["small", "small"]"#).unwrap_err();
    assert_eq!(problems.problems.get(0), Some(&crate::Problem { path: role_models_path(1), what: What::Unique }));
}
#[test]
fn each_owner_owns_every_seeded_project() {
    let engine = fixtures::engine(&two_projects(b'2')).unwrap();
    assert_eq!(engine.owners.len(), 2);
    assert_eq!(engine.owners[0].project, 1);
    assert_eq!(engine.owners[1].project, 2);
    assert_eq!(engine.owners[0].identity.subject.as_ref(), b"owner-a");
    assert_eq!(engine.owners[1].identity.subject.as_ref(), b"owner-a");
}
fn a_permission_naming_an_unknown_role_is_refused() {
    refuses(br#""permission": 1, "role": 1"#, br#""permission": 1, "role": 9"#, What::Refers);
}
fn a_project_policy_past_the_deployment_ceiling_is_refused() {
    refuses(
        br#""escalation_role": 1, "period_spend": 100"#,
        br#""escalation_role": 1, "period_spend": 101"#,
        What::Bound,
    );
}
#[test]
fn charter_instruction_files_are_named_with_the_available_policy_bytes() {
    let engine = engine();
    let files = crate::files(&engine);
    let file = instruction_file(&files);
    assert!(!file.private);
    assert_eq!(file.at, fixtures::path(&[b"charters"]).child(crate::Segment::Index(0)).member(b"instructions"));
    assert!(file.max < engine.limits.core.run_bytes);
}
fn a_message_bound_below_a_delegate_result_is_refused() {
    refuses(
        br#""profile": "small""#,
        br#""profile": "small", "overrides": {"tasks.result_bytes": 4097}"#,
        What::Message,
    );
}
#[test]
#[expect(
    clippy::disallowed_macros,
    clippy::disallowed_methods,
    reason = "the memory fixture writer is ordinary test Rust (programming-model.md, 10.2)"
)]
fn memory_below_the_components_sum_is_refused() {
    // Connector grants and acted tables/alarms are priced alongside party flights (people.md, 5.3).
    const BEFORE_FINISHING_OVER: u64 = 18_729_868_387;
    // Small's two pending sign-ins need two additional close outputs for cancellation
    // (protocol/llm.md, 5.2). Propagating that room through the core and journal
    // queues adds 1,617,424 bytes to this fixture's components sum.
    const SIGN_IN_CANCEL_GROWTH: u64 = 1_617_424;
    // Open ending authority grows the small profile's owned rows and carriers.
    const ENDING_AUTHORITY_GROWTH: u64 = 992;
    // The live ask grows the small profile's typed host carriers by 24 bytes.
    const LIVE_ASK_GROWTH: u64 = 24;
    // A finish flag grows StartMessage and held NamedEvent from 40 to 48 bytes.
    // Small has two slots, each with eight start messages and eight held events.
    const FINISH_MESSAGE_GROWTH: u64 = 2 * (8 + 8) * 8;
    // Live pieces price all 64 hub activity slots at the piece bound plus
    // Input's provider id and tool wire bounds (hosts.md, 6.5).
    const LIVE_PIECE_GROWTH: u64 = 64 * (4096 + 256 + 64);
    // Each of 8 preparations keeps 2048 bytes and 2048 24-byte text carriers.
    // Each of 18,590 routed payloads and 4 loads prices one more tail buffer.
    const TRANSCRIPT_TEXT_GROWTH: u64 = 8 * (2048 * 24 + 2048) + (18_590 + 4) * 2048;
    // Finishing (domain/root.md, section 3; domain/people.md, section 3):
    // people's sign_in_worst_case adds Set<(Wall, u64)> at 8 sign-ins: 576 bytes.
    // Core's Reconciliation scratch replaces the views' Token snapshot:
    // 24 - 8 = 16 bytes per retained run. Ending scratch replaces (Token,
    // WatchBinding) with (Token, End): 88 - 16 = 72 fewer bytes per watcher.
    // The small profile has 4 runs and 4 watchers: 576 + 64 - 288 = 352.
    // The 32-byte set header and 8-byte room cache are inline, outside this heap sum.
    const RECONCILIATION_GROWTH_PER_RUN: u64 = 24 - 8;
    const ENDING_SAVING_PER_WATCHER: u64 = 88 - 16;
    // Canonical source deadlines retain both clocks: State and Hold grow 16 -> 24,
    // Source 152 -> 168, and Fact 64 -> 80 bytes, all still aligned to 8 bytes.
    // Small has 4 * 3 sources: (12 / 5 + 1) = 3 B-tree nodes, each with 11 slots.
    // Node headers, keys and edges are unchanged; source slots add 3 * 11 * 16 = 528.
    // The 32 queued facts and deployment's 4 account facts add (32 + 4) * 16 = 576.
    // Inline container headers are unchanged; total growth is 528 + 576 = 1,104.
    const SOURCE_DEADLINE_GROWTH: u64 = 3 * 11 * (168 - 152) + (32 + 4) * (80 - 64);
    // A sign-in under way keeps its finish out, an Option<Token> of 16 bytes (its `taken`
    // flag fits the padding): small's 2 sign-ins fill one B-tree node of 11 slots.
    const PASTED_FINISH_GROWTH: u64 = 11 * 16;
    let document = fixtures::with(fixtures::ENGINE_SMALL, br#""memory_bytes": 34359738368"#, br#""memory_bytes": 1"#);
    let problems = fixtures::engine(&document).unwrap_err();
    // Worker program sources add 6,723,936 bytes: the hub's and the fleet's source records
    // and pending replacements, their channel and routed output room, and the journal
    // writes and KV operations an extra Sources output at hello needs.
    // Named authority selectors add 14,819,720 bytes over main: 12,632,064 for the
    // 32 answered and 16 pending payload name-context envelopes (48 * 263,168),
    // 526,336 for one routed Event and its people::Request::Route (2 * 263,168),
    // and 1,661,320 for native selector tags, name-list carriers and their derived
    // retained-storage and queue envelopes. People's pending clone is priced there.
    // x1c adds 0 bytes here: Account's alignment absorbs its now-required provider's
    // removed Option tag. Provider registrations are priced by the service.
    // JP10c-b adds 248 bytes over f448c8d4: 240 inline bytes for the parser and
    // two runtime provider copies, plus 8 bytes for their two "Fake" names.
    // This fixture has no manage address; supplied addresses cost three copies.
    // Over 3ca78043, x1d adds 345,296 bytes (people LlmChoice 24 -> 32; Member 360 -> 368;
    // Amendment 320 -> 328; Ask 360 -> 368; Task 1,464 -> 1,504). The pin was already priced.
    // People: 1 * 8 amendment scratch + 48 * (8 * 8) compound-ask payload copies
    // (32 answered + 2 * 8 pending), + 77 * 8 answered B-tree slots (7 nodes * 11),
    // + 8 * 16 pending inline asks = 3,824.
    // Documents: 20 * (56 * 40) = 44,800: one construction + 4 watches + 4 loads
    // + (4 rows + 3) read-answer copies + 4 act scratch; each Project envelope
    // prices (4 lines squared + 8 * 4 + 8) task carriers, each gaining the 40-byte wait.
    // Routed/load payloads: (18,538 writes + 4 loads) * (2 members * 8) = 296,672.
    // 3,824 + 44,800 + 296,672 = 345,296; hub unchanged.
    // JP10c-c keeps the document envelope unchanged (20 * 0); readiness scratch
    // now coexists with deployment's views: 2 * 32 Pending + 2 * 12 ProjectRole
    // + 4 * 8 Coverage + 4 * 4 candidate numbers, plus the largest temporary:
    // max(4 * 32 ranked candidates, (4 * 3) * 64 source Facts,
    //     (2 workers + 8 history + 0 engine) * 40 HostView + 1 * 24 ModelNeed
    //     + 1 * 4096 model-name bytes) = 4520.
    // Delta = 64 + 24 + 32 + 16 + 4520 = 4656; 18_677_344_295 + 4656
    // = 18_677_348_951. Other document scratch paths remain below deployment.
    // Before review, y1b added 114,476 bytes over main's 18_677_348_951:
    // Issued: 3 nodes * 600 + 12 names * 32 = 2,184.
    // Pending operations: 3 nodes * 512 + 12 sources * 9 hosts * 8,
    // plus 9 terminal rows * 8 = 2,472.
    // Removal unions: 4 accounts * 352 + 11 B-tree slots * 32 inline growth = 1,760.
    // Fleet: 2 channel slots * 72 + 2 workers * 3 pending nodes * 176 growth,
    // + 2 * 3 check nodes * 480 + 2 * 3 result nodes * 744
    // + 2 * 12 result details * 4,096 = 106,848.
    // One extra scratch list + cleanup names: 2 * 12 * 33 = 792;
    // target snapshot: (8 configured workers + 1 engine) * 4 = 36;
    // configured names: 8 * (16 carrier + 32 bytes) = 384.
    // 2,184 + 2,472 + 1,760 + 106,848 + 792 + 36 + 384 = 114,476.
    // These are the original envelopes; joined sign-out terminals change their
    // output room below. The engine has no program backend.
    // Facts add 416: 2 lost-history nodes * 11 slots * 8 bytes,
    // plus 3 host-view copies * (2 connected + 8 history rows) * 8 bytes.
    // Readiness's max(128 ranking, 768 facts, 4520 hosts/model) moves to
    // max(128, 768, 4600): 10 HostViews * (48 - 40) = 80 of those bytes.
    // Program documents have their own explicit bound but remain below the
    // pre-existing largest document envelope; routed and journal caps are unchanged.
    // Public answer validation adds one host-name scratch copy:
    // (8 configured workers + 1 engine) * (24-byte carrier + 32-byte name) = 504.
    // Retained answers take max(9 * 56, 2,048 visit bytes), so that term is unchanged.
    // Total y1b delta: 114,476 + 416 + 504 = 115,396 over main.
    // Review: typed Issued grows 40 -> 56 bytes, while twelve owned names disappear:
    // 3 B-tree nodes * 11 slots * 16 - 12 names * 32 = 144 bytes.
    // Atomic Pending grows 32 -> 56: 3 nodes * 11 slots * 24 = 792.
    // Joined fleet replies grow Pending 24 -> 32: 2 workers * 3 nodes * 11 * 8 = 528.
    // Worker and failure rows stay 8 bytes. Program becomes 34 inline bytes instead
    // of 24 + 32 owned; its envelope remains below the 638,368-byte routed payload.
    // Two replies per source lift fleet max_out from 21 to 25. Four scratch queues:
    // 4 queues * 4 extra requests * 88 bytes = 1,408.
    // Core route room grows 18,538 -> 18,570 (8 routes * 4 extra fleet outputs):
    // 32 * (536 Event + 240 Request + 638,368 owned payload) = 20,452,608.
    // Projection caches: 8 tasks * (2 connectors + 2) caches * 32 extra milestones
    // * (168 inline + 4,096 words + 2 * 512 result bytes) = 5,414,912.
    // Review delta = 144 + 792 + 528 + 1,408 + 20,452,608 + 5,414,912 = 25,870,392.
    // Re-review: the leaf keeps SourceName 33 bytes and Worker 4 bytes; Membership
    // replaces one 16-byte Box carrier with one 16-byte wrapper (1 * 0 bytes).
    // Names remain 8 * (16 carrier + 32 payload) = 384 bytes. Position's u32 fits
    // the existing HelloRefusal envelope: 8 history rows * 0 added bytes.
    // The one-pass collector uses the already priced 9 failures * 8 = 72 bytes.
    // No changed routed/document maximum and no change to the sum below.
    // 18,677,464,347 + 25,870,392 = 18,703,334,739; the sum is one byte greater.
    // JP10d adds 64 * (512 description bytes + 64 inline bytes) = 36,864 to
    // the clients' component, outside this core + hub + provider-terms sum.
    // The clients' Model grows from 48 to 112 bytes: Prices is 48 bytes and the
    // optional description's slice header is 16. List::worst_case prices that growth.
    // JP12: Form grows 1 -> 34 bytes; aligned Granted, Source, Kept and Grant
    // each grow 32 bytes. Accounts: (3 source nodes + 1 credential node) * 11
    // B-tree slots * 32 = 1,408. Fleet: 12 tracked (8 attempts + 2 workers * 2 slots)
    // * 8 grants * 32 = 3,072. Core: 2 copies * 8 tasks * 8 grants * 32 = 4,096.
    // Hub: 2 slots * 8 accounts * 32 = 512. Total: 284 * 32 = 9,088.
    // 18,703,334,740 + 9,088 = 18,703,343,828; one byte is supplied.
    // The placement check adds 4 accounts * 3 sources * 8-byte HostSource = 96 bytes.
    // Catalogue scratch keeps those same 12 facts beside the host/model walk: another 96 bytes.
    // HostRule adds 4 host lines * 16 aligned bytes = 64 to the deployment envelope.
    // It remains below the Project envelope (56 overview carriers), so the components sum does not move
    // for that field. The two scratch lists above add 192 bytes over 18,703,343,827.
    // JP12 commit 3: 64 configured models * (16-byte optional program carrier +
    // 32-byte backend name) + 4 charter envelopes * 16 bytes = 3,136 bytes.
    // Run-policy payloads stay within their existing run_bytes envelopes.
    // Added dependencies (including the seed's task/run carriers), retained
    // refusal names, circle scratch and document facts add 21,322,048 bytes.
    // The profile is unchanged and still supplies one byte in this refusal test.
    // The tasks domain's wait index, its circle scratch and an amendment's staged
    // promise (tasks::worst_case at this profile's tasks limits) add 2,100,096:
    // 18,724,669,203 + 2,100,096 = 18,726,769,299.
    // The circle judge (waits::worst_case and compound::worst_case) adds 289,872:
    // wait rows naming their delegates 352 plus 512 comparison bytes, a third
    // set 400, two u32 maps for two u64 maps -160, knot waits 1,024, knot
    // witnesses 8,192 and their 256 owned names 278,528, and a staged return's
    // 256 circle slots and 768 projected bytes: 18,726,769,299 + 289,872 = 18,727,059,171.
    // Core result reads add 2,171,840 bytes: the map for 8 reads is
    // 2 B-tree nodes * (16 header + 11 * (8 token + 72 read) + 12 * 8 edges) = 1,984.
    // Each read holds 8 dependencies + 8 inputs: 16 * (8 ID + 48 HistoricalResult
    // + 512 result bytes * (32 ResultField + 1 payload byte)) = 271,232.
    // 1,984 + 8 reads * 271,232 = 2,171,840;
    // 18,727,059,171 + 2,171,840 = 18,729,231,011.
    // An amendment's ended-dependency read adds 349,360 bytes. ReadFor gains its
    // amendment arm, 16 -> 24 bytes, so each read is 80: 2 nodes * 11 * 8 = 176.
    // Each of the 8 tasks' amendment reads holds 456 bytes inline, 8 dependencies *
    // (8 depth + 1) = 72 places * 48 = 3,456 and 72 IDs * 8 = 576, an 8-byte token,
    // and its replay: max(30,960 task carrier + 4,096 spec + 4,096 message, 8,192
    // amendment) = 39,152; 8 * 43,648 = 349,184.
    // 18,729,231,011 + 176 + 349,184 = 18,729,580,371 (one byte is supplied).
    // Question positions' ordering scratch and the core's bounded activity
    // cache, pruning keys and census keys add 288,016 bytes at this profile.
    let limits = crate::profile(crate::Profile::Small).core;
    let wall_expiry_index = skein_lib::Set::<(skein_lib::Wall, u64)>::worst_case(limits.people.sign_ins).unwrap();
    let reconciliation_growth = u64::from(limits.views.runs) * RECONCILIATION_GROWTH_PER_RUN;
    let ending_saving = u64::from(limits.views.watchers) * ENDING_SAVING_PER_WATCHER;
    let without_acts = BEFORE_FINISHING_OVER + wall_expiry_index + reconciliation_growth - ending_saving
        + SOURCE_DEADLINE_GROWTH
        + SIGN_IN_CANCEL_GROWTH
        + PASTED_FINISH_GROWTH
        + ENDING_AUTHORITY_GROWTH
        + LIVE_ASK_GROWTH
        + FINISH_MESSAGE_GROWTH
        + LIVE_PIECE_GROWTH
        + TRANSCRIPT_TEXT_GROWTH
        // Deployment role storage and project-role scratch: people.projects 2 -> 3.
        + 7_288
        // Project names: (18,570 route + 4 load) payloads * 64 bytes,
        // a 336-byte map and two 64-byte names, plus two codec scratch copies.
        + 1_189_328
        // Owner project name: 20 document reservations * (24-byte Text + 4,096 payload bytes).
        + 82_400
        // The first start's seeds and the configured policy table: their slots, and each
        // policy at `policy_bytes` (a seed with its name), as the rows cost.
        + 68_576
        // Account moves add bounded move/arrival tables, payloads and deadlines,
        // and enlarge the accounts output bound and the core's routed envelopes.
        + 14_560_734
        // Stored settings' two sets, restore transients and fleet catalogue evidence.
        + 2_010_752
        // Owner settings changes retain a bounded input and decision scratch.
        + 1_880
        // Settings reads: the inventory document and its scratch, revision and history loads
        // with their maps, the stored catalogue's effort names, and the settings holders' counts.
        + 5_810_584
        // Run settings reads: Destination grows 16 -> 48 bytes for ReplyTo + CallKey,
        // across small's four Pending load slots: 4 * 32 = 128. The inline
        // SettingsAnswer carrier is 80 bytes, replacing Answer's 24 in the one
        // constructed-answer envelope: another 56. No new table or limit.
        + 4 * 32 + 56
        // Run proofs' outstanding read fences (tasks.md 9): (4 proofs per task + 2) at
        // 8 run-inbox ids, a routed payload's id list, and the wider inline RunProof.
        // M3's CurrentWindow uses bool's niche: Option<(u64, u32)> 24 ->
        // Option<CurrentWindow> 16; RunProof, Record and Request 256 -> 248.
        // The routed Request queue saves (18,590 + 1) * 8 = 148,728 bytes.
        // Read Row holds value and related Record carriers: 680 -> 664. Each load
        // saves 4 rows * (16 Row + 8 Record) = 96; the incoming page and two
        // answer copies save 3 * 96 = 288 (the settings load still exceeds 65,536).
        // Core: 18,789,463,490 -> 18,789,314,474; hub/provider terms unchanged.
        // 1,490,224 - 148,728 - 288 = 1,341,208; no store share enters this sum.
        + 1_341_208
        // Connector hold accounts and their wider typed task/document/route carriers.
        + 789_448
        // Eight tasks, each with three retained/scratch records, gain a 16-byte room wait.
        + 8 * 3 * 16;
    // Regenerate this charge with JIG_MEMORY_WRITE=1 through heavy, then run
    // normally to check it. The unchanged terms above keep the act saving local.
    let charge_path = skein_scratch::paths::package().join("src/tests/deployment_act_growth.txt");
    if std::env::var_os("JIG_MEMORY_WRITE").is_some_and(|value| value == "1") {
        let What::Memory { over } = problems.problems.get(0).unwrap().what else { panic!("memory refusal") };
        std::fs::write(&charge_path, format!("{}\n", over.checked_sub(without_acts).unwrap())).unwrap();
    }
    let growth: u64 = std::fs::read_to_string(charge_path).unwrap().trim().parse().unwrap();
    assert_eq!(problems.problems.get(0).unwrap().what, What::Memory { over: without_acts + growth });
}
fn instruction_file(files: &[crate::File]) -> &crate::File {
    for file in files {
        if file.path.as_ref() == b"charters/chat.txt" {
            return file;
        }
    }
    panic!("instruction file configured")
}
#[test]
fn resource_grants_read_their_literal_pattern() {
    let engine = fixtures::engine(&with_grants(b'1')).unwrap();
    assert_eq!(engine.rules.ceiling.grants[0].pattern.segments.as_ref(), &[Box::<[u8]>::from(&b"service"[..])]);
    assert_eq!(engine.rules.ceiling.grants[0].pattern.last, jig_domain_authority::Last::Open(Box::default()));
}
#[test]
fn a_grant_naming_an_unknown_connector_is_refused() {
    let problems = fixtures::engine(&with_grants(b'2')).unwrap_err();
    assert_eq!(problems.problems.get(0).unwrap().what, What::Refers);
}
#[test]
fn a_requirement_reads_its_judge_and_freshness() {
    let requirement = br#""requirements": [{"connector": 1,"kind":2,"pattern":{"segments":[],"last":"open","value":""},"judge":{"connector":1,"requirement":3,"parameters":4},"guard":"observed","freshness_ms":5000,"must_be_guarded":false}]"#;
    let document = fixtures::with(fixtures::ENGINE_SMALL, br#""requirements": []"#, requirement);
    let engine = fixtures::engine(&document).unwrap();
    assert_eq!(engine.rules.requirements[0].judge.parameters, 4);
    assert_eq!(
        engine.rules.requirements[0].guard,
        jig_domain_authority::Guard::Observed { freshness: skein_lib::Duration::from_secs(5) }
    );
}
fn an_implication_table_not_transitively_closed_is_refused() {
    refuses(
        br#""implies": []"#,
        br#""implies": [{"connector":1,"kind":1,"implies":2},{"connector":1,"kind":2,"implies":3}]"#,
        What::Bound,
    );
}

#[test]
fn domain_configuration_problems_name_the_invalid_fields() {
    table([
        ("an_owner_naming_an_unknown_provider_is_refused", an_owner_naming_an_unknown_provider_is_refused),
        ("a_message_bound_below_a_partys_words_is_refused", a_message_bound_below_a_partys_words_is_refused),
        ("a_retention_member_is_refused_as_unknown", a_retention_member_is_refused_as_unknown),
        ("a_projects_resources_member_is_refused_as_unknown", a_projects_resources_member_is_refused_as_unknown),
        ("a_projects_homes_member_is_refused_as_unknown", a_projects_homes_member_is_refused_as_unknown),
        ("two_deployment_providers_are_refused", two_deployment_providers_are_refused),
        ("a_deployment_provider_is_required", a_deployment_provider_is_required),
        ("a_connector_numbered_as_the_cores_own_is_refused", a_connector_numbered_as_the_cores_own_is_refused),
        ("a_permission_naming_an_unknown_role_is_refused", a_permission_naming_an_unknown_role_is_refused),
        (
            "a_project_policy_past_the_deployment_ceiling_is_refused",
            a_project_policy_past_the_deployment_ceiling_is_refused,
        ),
        ("a_message_bound_below_a_delegate_result_is_refused", a_message_bound_below_a_delegate_result_is_refused),
        (
            "an_implication_table_not_transitively_closed_is_refused",
            an_implication_table_not_transitively_closed_is_refused,
        ),
        ("a_ceiling_naming_an_unknown_model_is_refused", a_ceiling_naming_an_unknown_model_is_refused),
        ("a_ceiling_naming_a_model_twice_is_refused", a_ceiling_naming_a_model_twice_is_refused),
        ("a_role_naming_an_unknown_model_is_refused", a_role_naming_an_unknown_model_is_refused),
        ("a_role_naming_a_model_twice_is_refused", a_role_naming_a_model_twice_is_refused),
    ]);
}

type Case = (&'static str, fn());

fn table<const N: usize>(cases: [Case; N]) {
    let mut failures = [None; N];
    let mut passed = true;
    for ((name, case), failure) in cases.into_iter().zip(&mut failures) {
        if std::panic::catch_unwind(case).is_err() {
            *failure = Some(name);
            passed = false;
        }
    }
    assert!(passed, "{failures:?}");
}

#[test]
fn the_catalogue_key_bound_follows_the_component() {
    let mut engine = engine();
    engine.limits.driver.client.document.token_bytes = 123;
    let core = crate::core(&engine, [0; 16], 1);
    assert_eq!(core.settings.key_bytes, engine.limits.driver.client.document.token_bytes);
}
#[test]
fn the_larger_carried_value_bound_keeps_the_catalogue_key_bound() {
    let engine = engine();
    let core = crate::core(&engine, [0; 16], 1);
    assert_eq!(core.settings.key_bytes, engine.limits.driver.client.document.token_bytes);
    assert!(core.settings.key_bytes < engine.limits.clients.wire.keyed_value_some);
}

#[test]
fn a_seed_policy_whose_whole_value_exceeds_its_byte_limit_refuses_start() {
    // Permission mappings fit 256 bytes, but the role and whole ceiling do not.
    refuses(br#""profile": "small""#, br#""profile": "small", "overrides": {"core.policy_bytes": 256}"#, What::Bound);
}

// Current people-v13 envelope, configuration.md, section 6.
const POLICY_MAX: u64 = 43_695;
const AMENDMENT_MAX: u32 = 8_192;
const LARGE_POLICY: u64 = 65_536;

#[test]
fn shipped_profiles_fit_the_effective_codec_envelope() {
    for profile in [crate::Profile::Small, crate::Profile::Standard] {
        let limits = crate::profile(profile).core;
        assert_eq!(crate::check_effective_limits(&limits), Ok(()), "{profile:?}");
        let derived = jig_protocol_store::codec::people::derived_limits(&limits).unwrap();
        assert_eq!(jig_protocol_store::people_v13::check_limits(&derived), Ok(()));
    }
}

#[test]
fn exported_maxima_are_the_last_codec_admitted_values() {
    for profile in [crate::Profile::Small, crate::Profile::Standard] {
        let limits = crate::profile(profile).core;
        assert_eq!(crate::policy_bytes_max(&limits), Some(POLICY_MAX), "{profile:?}");
        assert_eq!(crate::amendment_bytes_max(&limits), Some(AMENDMENT_MAX), "{profile:?}");
        let mut policy = limits;
        policy.policy_bytes = POLICY_MAX;
        assert_eq!(crate::check_effective_limits(&policy), Ok(()));
        assert!(jig_protocol_store::codec::limits(&policy).is_some());
        let mut amendment = limits;
        amendment.people.amendment_bytes = AMENDMENT_MAX;
        assert_eq!(crate::check_effective_limits(&amendment), Ok(()));
        assert!(jig_protocol_store::codec::limits(&amendment).is_some());
    }
}

fn policy_problem(value: u64) -> crate::Problem {
    let mut limits = crate::profile(crate::Profile::Small).core;
    limits.policy_bytes = value;
    crate::check_effective_limits(&limits).expect_err("policy exceeds the codec envelope")
}

#[test]
fn policy_past_the_envelope_names_its_binding_field() {
    let problem = policy_problem(LARGE_POLICY);
    assert_eq!(problem.path, fixtures::path(&[b"limits", b"core", b"policy_bytes"]));
    assert_eq!(
        problem.what,
        What::PolicyEnvelope {
            value: LARGE_POLICY,
            maximum: POLICY_MAX,
            bound: Some(crate::CodecBound { field: b"PolicyValue.permissions", needed: 98_304, ceiling: 65_536 }),
        }
    );
    assert_eq!(problem.codec_message().unwrap().as_ref(), b"limits.core.policy_bytes: 65536 exceeds the people-v13 codec envelope of 43695 bytes (PolicyValue.permissions would need 98304 bytes; ceiling 65536).");
}

#[test]
fn the_next_policy_byte_is_refused_at_the_binding_field() {
    let problem = policy_problem(POLICY_MAX + 1);
    assert_eq!(
        problem.what,
        What::PolicyEnvelope {
            value: POLICY_MAX + 1,
            maximum: POLICY_MAX,
            bound: Some(crate::CodecBound { field: b"PolicyValue.permissions", needed: 65_544, ceiling: 65_536 }),
        }
    );
}

#[test]
fn the_next_amendment_byte_is_refused_with_the_agreed_words() {
    let mut limits = crate::profile(crate::Profile::Small).core;
    limits.people.amendment_bytes = AMENDMENT_MAX + 1;
    let problem = crate::check_effective_limits(&limits).unwrap_err();
    assert_eq!(problem.path, fixtures::path(&[b"limits", b"people", b"amendment_bytes"]));
    assert_eq!(
        problem.what,
        What::AmendmentEnvelope {
            value: AMENDMENT_MAX + 1,
            maximum: AMENDMENT_MAX,
            bound: Some(crate::CodecBound { field: b"Segment.bytes", needed: 8_193, ceiling: 8_192 }),
        }
    );
    assert_eq!(
        problem.codec_message().unwrap().as_ref(),
        b"limits.people.amendment_bytes: 8193 exceeds the people-v13 codec envelope of 8192 bytes."
    );
}

#[test]
fn an_independent_ceiling_names_the_generated_first_failure() {
    let mut limits = crate::profile(crate::Profile::Small).core;
    limits.people.identity_bytes = 8_193;
    let problem = crate::check_effective_limits(&limits).unwrap_err();
    assert_eq!(problem.path, fixtures::path(&[b"limits", b"people", b"identity_bytes"]));
    assert_eq!(
        problem.what,
        What::CodecCeiling {
            bound: crate::CodecBound { field: b"IdentityKey.subject", needed: 8_193, ceiling: 8_192 },
        }
    );
    assert_eq!(crate::policy_bytes_max(&limits), None);
    assert_eq!(crate::amendment_bytes_max(&limits), None);
    assert_eq!(
        problem.codec_message().unwrap().as_ref(),
        b"limits.people.identity_bytes: IdentityKey.subject would need 8193; people-v13 codec ceiling 8192."
    );
}

#[test]
fn arithmetic_endpoints_are_refused_without_panicking() {
    let mut limits = crate::profile(crate::Profile::Small).core;
    limits.policy_bytes = u64::MAX;
    let problem = crate::check_effective_limits(&limits).unwrap_err();
    assert_eq!(problem.path, fixtures::path(&[b"limits", b"core", b"policy_bytes"]));
    assert_eq!(problem.what, What::PolicyEnvelope { value: u64::MAX, maximum: POLICY_MAX, bound: None });
    assert!(jig_protocol_store::codec::people::derived_limits(&limits).is_none());
    limits.policy_bytes = 16_384;
    limits.people.amendment_bytes = u32::MAX;
    let problem = crate::check_effective_limits(&limits).unwrap_err();
    assert_eq!(problem.path, fixtures::path(&[b"limits", b"people", b"amendment_bytes"]));
    assert_eq!(problem.what, What::AmendmentEnvelope { value: u32::MAX, maximum: AMENDMENT_MAX, bound: None });
}

#[test]
fn maxima_hold_the_other_budget_fixed_even_when_it_is_refused() {
    let mut limits = crate::profile(crate::Profile::Small).core;
    limits.people.amendment_bytes = AMENDMENT_MAX + 1;
    assert_eq!(crate::policy_bytes_max(&limits), None);
    limits.people.amendment_bytes = AMENDMENT_MAX;
    limits.policy_bytes = POLICY_MAX + 1;
    assert_eq!(crate::amendment_bytes_max(&limits), None);
}

#[test]
fn two_excessive_budgets_report_policy_then_amendment() {
    let mut limits = crate::profile(crate::Profile::Small).core;
    limits.policy_bytes = LARGE_POLICY;
    limits.people.amendment_bytes = AMENDMENT_MAX + 1;
    let policy = crate::check_effective_limits(&limits).unwrap_err();
    assert_eq!(policy, policy_problem(LARGE_POLICY));
    limits.policy_bytes = POLICY_MAX;
    let amendment = crate::check_effective_limits(&limits).unwrap_err();
    assert_eq!(amendment.path, fixtures::path(&[b"limits", b"people", b"amendment_bytes"]));
}

#[test]
fn other_store_families_are_checked_after_the_people_probe() {
    let mut limits = crate::profile(crate::Profile::Small).core;
    // The core-v6 SettledAnswer bytes ceiling is independent of the people codec.
    limits.call_answer_bytes = u32::MAX;
    assert!(jig_protocol_store::codec::people::limits(&limits).is_some());
    assert_eq!(
        crate::check_effective_limits(&limits),
        Err(crate::Problem { path: fixtures::path(&[b"limits"]), what: What::StoreCodec })
    );
}

#[test]
fn independent_item_ceilings_keep_their_own_field_and_bound() {
    let mut limits = crate::profile(crate::Profile::Small).core;
    limits.authority.roles = 9;
    let problem = crate::check_effective_limits(&limits).unwrap_err();
    assert_eq!(problem.path, fixtures::path(&[b"limits", b"authority", b"roles"]));
    assert_eq!(
        problem.what,
        What::CodecCeiling { bound: crate::CodecBound { field: b"PolicyValue.roles", needed: 9, ceiling: 8 } }
    );
    assert_eq!(
        problem.codec_message().unwrap().as_ref(),
        b"limits.authority.roles: PolicyValue.roles would need 9; people-v13 codec ceiling 8."
    );
}
