use skein_codegen::{generate, parse};
use std::fs;

#[test]
fn moved_schema_matches_generated_code_and_goldens() {
    let root = skein_scratch::paths::workspace().join("crates/jig-protocol-accounts");
    let source = fs::read_to_string(root.join("schema/moved-v1.schema")).expect("schema");
    let schema = parse(&source).expect("schema valid");
    let output = generate(&schema);
    let code = root.join("src/generated/v1.rs");
    let goldens = root.join("golden");
    let write = std::env::var_os("SKEIN_CODEGEN_WRITE").is_some_and(|value| value == "1");
    let again = "run SKEIN_CODEGEN_WRITE=1 heavy cargo nextest run -p jig-protocol-accounts-world --test drift";
    if write {
        fs::write(&code, &output.rust).expect("write generated code");
        fs::create_dir_all(&goldens).expect("create golden directory");
    } else {
        assert_eq!(fs::read_to_string(&code).expect("generated code"), output.rust, "{again}");
    }
    for golden in output.goldens {
        let path = goldens.join(&golden.name);
        if write {
            fs::write(&path, golden.bytes).expect("write golden bytes");
        } else {
            assert_eq!(fs::read(&path).expect("golden bytes"), golden.bytes, "golden {} drifted: {again}", golden.name);
        }
    }
}
