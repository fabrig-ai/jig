//! Frame capacities measured independently of the maps (store.md 8.6, 13).
#![expect(
    clippy::disallowed_macros,
    clippy::print_stderr,
    reason = "the measurement reads a pinned fixture and reports its numbers"
)]

use crate::transcripts::patches;
use skein_codec::{patch::Patcher, schema};

// Smith's schema at jig's smith pin, checked against smith-transcript's
// generated code by `the_real_copy_is_smiths_schema`.
const REAL: &str = include_str!("fixtures/smith-transcript-v6.schema");
// Smith's real turn alone, as its set keeps it from the baseline on.
const REAL_SETS: crate::PatchSets = crate::PatchSets {
    sets: &[crate::PatchSet {
        patches: &[],
        fresh: None,
        families: &[crate::KeptFamily {
            name: smith_transcript::FAMILY,
            texts: &[REAL],
            frames: Some(smith_transcript::ROOT),
            ..super::frame_patches::SETS.sets[1].families[0]
        }],
        ..super::frame_patches::SETS.sets[1]
    }],
};
const CANDIDATES: [u32; 4] = [4096, 16_384, 65_536, 262_144];

// 20 envelope/message + 2*4 text + 16*6 calls + 16*14 read results.
const TYPICAL_NODES: u32 = 348;
#[test]
fn frame_capacities_cover_real_turns_with_small_serving_cost() {
    let profile = jig_protocol_config::profile(jig_protocol_config::Profile::Small);
    let source = profile.store;
    let limits = crate::Limits {
        core: source.core,
        store: source.store,
        secrets: source.secrets,
        journal: source.journal,
        joins: source.joins,
        queue: source.queue,
        files: source.files,
        reads: source.reads,
        orphan_erases: source.orphan_erases,
        held: source.held,
    };
    let sets = super::frame_patches::SETS;
    let source = jig_store_world::patches::limits();
    let world = crate::Limits {
        core: source.core,
        store: source.store,
        secrets: source.secrets,
        journal: source.journal,
        joins: source.joins,
        queue: source.queue,
        files: source.files,
        reads: source.reads,
        orphan_erases: source.orphan_erases,
        held: source.held,
    };
    eprintln!(
        "world serving before {}, priced {}, frame machines {}",
        crate::worst_case(&world).expect("world"),
        crate::worst_case_with_patches(&world, &sets).expect("priced"),
        patches::worst_case(&world, &sets).expect("machines")
    );
    for (label, configuration) in [("world", &world), ("small", &limits)] {
        let sets = super::baseline::SETS;
        let before = crate::worst_case(configuration).expect("before baseline");
        let priced = crate::worst_case_with_patches(configuration, &sets).expect("real sets priced");
        let frames = patches::worst_case(configuration, &sets).expect("real frame machines");
        assert!(frames > 0, "both real frame families have kept text");
        eprintln!("real {label} serving before {before}, computed {priced}, frame machines {frames}");
    }
    let baseline = crate::worst_case(&limits).expect("baseline");
    let priced = crate::worst_case_with_patches(&limits, &sets).expect("priced");
    let machines = patches::worst_case(&limits, &sets).expect("machines");
    eprintln!("small serving before {baseline}, priced {priced}, frame machines {machines}");
    for nodes in CANDIDATES {
        let mut capacity = patches::capacity(&sets.sets[1], 0, &limits).expect("turn capacity");
        capacity.nodes = nodes;
        eprintln!(
            "smith nodes {nodes}: {} bytes (versions {}, patches {}, text {}, bytes {})",
            Patcher::worst_case_heap(capacity).expect("heap"),
            capacity.versions,
            capacity.patches,
            capacity.text,
            capacity.bytes
        );
    }
    let schema = schema::parse(REAL.as_bytes()).expect("smith's pinned schema");
    let root = schema.find(smith_transcript::ROOT.as_bytes()).expect("Turn");
    kept_turns_fit(&sets, &limits);
    let real = patches::capacity(&REAL_SETS.sets[0], 0, &limits).expect("real turn capacity");
    let maximum = maximum_decl(&schema, root);
    assert_eq!(real.nodes, maximum.0, "smith's machine reads every turn its kept schema allows");
    assert!(u64::from(maximum.1) <= u64::from(real.bytes), "the maximum-node turn fits the transcript's bytes");
    eprintln!("real smith Turn maximum nodes {}, achieved in {} bytes", real.nodes, maximum.1);
    let typical = typical_turn();
    let mut value = skein_codec::value::Value::with_room(TYPICAL_NODES);
    value.decode(&schema, root, typical.clone()).expect("typical tool-heavy turn fits its node count");
    let mut smaller = skein_codec::value::Value::with_room(TYPICAL_NODES.checked_sub(1).expect("nonempty"));
    assert_eq!(smaller.decode(&schema, root, typical.clone()), Err(skein_codec::value::Refusal::Room));
    eprintln!("typical smith 34-block tool-heavy turn {} bytes, {TYPICAL_NODES} nodes", typical.len());
    eprintln!("real smith machine heap {}", Patcher::worst_case_heap(real).expect("real heap"));
    let real_machines = patches::worst_case(&limits, &REAL_SETS).expect("real machines");
    eprintln!("real smith machine priced with its box {real_machines}");
    assert!(real_machines <= baseline / 10, "frames are at most ten percent of serving");
}

fn kept_turns_fit(sets: &crate::PatchSets, limits: &crate::Limits) {
    for set in sets.sets {
        if set.owner != crate::Owner::Smith {
            continue;
        }
        for (ix, family) in set.families.iter().enumerate() {
            let Some(root) = family.frames else { continue };
            let capacity = patches::capacity(set, ix, limits).expect("kept capacity");
            let meta = crate::patching::machines::kept_patcher(set, ix, capacity).expect("kept metadata");
            let last = schema::parse(family.texts.last().expect("kept last").as_bytes()).expect("last text").version();
            let mut most = 0;
            for text in family.texts {
                let schema = schema::parse(text.as_bytes()).expect("kept text");
                let root = crate::patching::walk::held_root(&meta, schema.version(), last, root.as_bytes())
                    .expect("kept root");
                most = most.max(maximum_decl(&schema, root).0);
            }
            assert_eq!(capacity.nodes, most, "the machine's nodes are its kept texts' most");
        }
    }
}

#[test]
fn the_real_copy_is_smiths_schema() {
    let schema = schema::parse(REAL.as_bytes()).expect("smith's pinned schema");
    let generated = generated_bounds();
    assert_eq!(copied_bounds(&schema), (generated.len(), generated), "the copy's bounds are smith's");
    let root = schema.find(smith_transcript::ROOT.as_bytes()).expect("Turn");
    assert_eq!(
        Some(u64::from(schema.most_bytes(root))),
        smith_transcript::Turn::worst_case_bytes(&smith_transcript::CEILINGS),
        "the copy's worst case is smith's"
    );
    assert_eq!(u64::from(schema.version()), smith_transcript::Turn::FORMAT_VERSION);
    assert_eq!(schema.spelled(schema.family()), smith_transcript::FAMILY.as_bytes());
}

// The bounds smith-transcript's generated `Limits` names.
const SMITH_BOUNDS: usize = 34;

/// Every bound smith's codegen names, in its declarations' order; a field
/// added or removed at a repin fails to compile here.
fn generated_bounds() -> [u32; SMITH_BOUNDS] {
    let smith_transcript::v6::Limits {
        replay_bytes,
        text_text,
        opaque_bytes,
        call_id,
        call_name,
        call_input,
        entry_name,
        hit_path,
        hit_text,
        read_content,
        listed_entries,
        found_hits,
        command_end_head,
        command_end_last,
        command_running_head,
        command_running_last,
        job_read_head,
        job_read_last,
        ambiguous_lines,
        working_path_path,
        other_path_directory,
        other_path_path,
        outside_path_path,
        field_problem_field,
        said_text,
        call_name_name,
        allowed_calls_names,
        tool_result_id,
        message_blocks,
        program_name,
        turn_endpoint,
        turn_model,
        turn_effort_some,
        turn_messages,
    } = smith_transcript::CEILINGS;
    [
        replay_bytes,
        text_text,
        opaque_bytes,
        call_id,
        call_name,
        call_input,
        entry_name,
        hit_path,
        hit_text,
        read_content,
        listed_entries,
        found_hits,
        command_end_head,
        command_end_last,
        command_running_head,
        command_running_last,
        job_read_head,
        job_read_last,
        ambiguous_lines,
        working_path_path,
        other_path_directory,
        other_path_path,
        outside_path_path,
        field_problem_field,
        said_text,
        call_name_name,
        allowed_calls_names,
        tool_result_id,
        message_blocks,
        program_name,
        turn_endpoint,
        turn_model,
        turn_effort_some,
        turn_messages,
    ]
}

/// The copy's bounds in the same order: each record field's first bound
/// past its options.
fn copied_bounds(schema: &schema::Schema) -> (usize, [u32; SMITH_BOUNDS]) {
    let mut copied = [0_u32; SMITH_BOUNDS];
    let mut count = 0_usize;
    for decl in 0..schema.declarations() {
        let schema::Shape::Record { first, count: fields, .. } = schema.declaration(schema::DeclIx::new(decl)).shape
        else {
            continue;
        };
        for offset in 0..fields {
            let mut ty = schema.field(schema::FieldIx::new(first.index().checked_add(offset).expect("field"))).ty;
            for _ in 0..schema.text().len() {
                match schema.ty(ty) {
                    schema::Type::Option(item) => ty = item,
                    schema::Type::List { bound, .. }
                    | schema::Type::Bytes { bound, .. }
                    | schema::Type::Text(bound)
                    | schema::Type::Secret { bound, .. } => {
                        *copied.get_mut(count).expect("no more bounds than smith's") = bound;
                        count = count.checked_add(1).expect("bounds");
                        break;
                    }
                    schema::Type::U8
                    | schema::Type::U16
                    | schema::Type::U32
                    | schema::Type::U64
                    | schema::Type::Bool
                    | schema::Type::Duration
                    | schema::Type::Fixed(_)
                    | schema::Type::Named(_) => break,
                }
            }
        }
    }
    (count, copied)
}

fn maximum_decl(schema: &schema::Schema, decl: schema::DeclIx) -> (u32, u32) {
    use schema::{FieldIx, Shape, VariantIx};
    match schema.declaration(decl).shape {
        Shape::Record { versioned, first, count } => {
            let mut total = (1_u32, if versioned { 2_u32 } else { 0 });
            for offset in 0..count {
                let field = schema.field(FieldIx::new(first.index().checked_add(offset).expect("field")));
                let child = maximum_type(schema, field.ty);
                total.0 = total.0.checked_add(child.0).expect("nodes");
                total.1 = total.1.checked_add(child.1).expect("bytes");
            }
            total
        }
        Shape::Enum { first, count } => {
            let mut best = (0, 0);
            for offset in 0..count {
                if let Some(decl) =
                    schema.variant(VariantIx::new(first.index().checked_add(offset).expect("variant"))).record
                {
                    let child = maximum_decl(schema, decl);
                    if child.0 > best.0 || (child.0 == best.0 && child.1 < best.1) {
                        best = child;
                    }
                }
            }
            (best.0.checked_add(1).expect("enum"), best.1.checked_add(1).expect("tag"))
        }
    }
}
fn maximum_type(schema: &schema::Schema, ty: schema::TypeIx) -> (u32, u32) {
    use schema::Type;
    match schema.ty(ty) {
        Type::Named(decl) => maximum_decl(schema, decl),
        Type::List { bound, item } => {
            let child = maximum_type(schema, item);
            (
                child.0.checked_mul(bound).expect("nodes").checked_add(1).expect("list"),
                child.1.checked_mul(bound).expect("bytes").checked_add(4).expect("count"),
            )
        }
        Type::Option(item) => {
            let child = maximum_type(schema, item);
            (child.0.checked_add(1).expect("option"), child.1.checked_add(1).expect("presence"))
        }
        Type::Fixed(size) => (1, size),
        Type::U8 | Type::Bool => (1, 1),
        Type::U16 => (1, 2),
        Type::U32 | Type::Bytes { .. } | Type::Secret { .. } | Type::Text(_) => (1, 4),
        Type::U64 | Type::Duration => (1, 8),
    }
}
fn typical_turn() -> Box<[u8]> {
    // 47 envelope/message bytes + 2 * 11 text blocks + 16 * (27 call + 65 read-result).
    const TYPICAL_BYTES: usize = 1541;
    let mut bytes = skein_lib::Writer::new(TYPICAL_BYTES);
    bytes.put(&6_u16.to_be_bytes()).expect("version");
    bytes.put(&[0, 0]).expect("kind, opening");
    bytes.put(&0_u32.to_be_bytes()).expect("endpoint");
    bytes.put(&0_u32.to_be_bytes()).expect("model");
    bytes.put(&[0]).expect("effort");
    bytes.put(&0_u32.to_be_bytes()).expect("dialect");
    bytes.put(&1_u32.to_be_bytes()).expect("place");
    bytes.put(&[0; 5]).expect("usage");
    bytes.put(&0_u64.to_be_bytes()).expect("spent");
    bytes.put(&0_u32.to_be_bytes()).expect("jobs");
    bytes.put(&1_u32.to_be_bytes()).expect("messages");
    bytes.put(&[1]).expect("assistant");
    bytes.put(&34_u32.to_be_bytes()).expect("blocks");
    text_block(&mut bytes);
    for _ in 0_u8..16 {
        bytes.put(&[2]).expect("call block");
        content(&mut bytes, b"tool-id");
        content(&mut bytes, b"read");
        content(&mut bytes, b"{}");
        bytes.put(&[0]).expect("replay");
        bytes.put(&[3]).expect("result block");
        content(&mut bytes, b"tool-id");
        bytes.put(&[0, 0]).expect("returned outcome, read");
        content(&mut bytes, &[b'a'; 32]);
        for _ in 0_u8..3 {
            bytes.put(&1_u32.to_be_bytes()).expect("line fields");
        }
        bytes.put(&[1, 0, 0]).expect("end, whole, within");
    }
    text_block(&mut bytes);
    bytes.finish()
}
fn text_block(bytes: &mut skein_lib::Writer) {
    bytes.put(&[0]).expect("text block");
    content(bytes, b"hello");
    bytes.put(&[0]).expect("no replay");
}
fn content(bytes: &mut skein_lib::Writer, text: &[u8]) {
    bytes.put(&u32::try_from(text.len()).expect("text").to_be_bytes()).expect("length");
    bytes.put(text).expect("content");
}
