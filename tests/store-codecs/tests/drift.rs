//! The committed codecs and golden bytes against their schemas (skein's codec.md, 6).
use skein_codegen::{generate, parse};
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;

fn store() -> PathBuf {
    skein_scratch::paths::workspace().join("crates/jig-protocol-store")
}

fn check(family: &str, version: u16) {
    let source =
        fs::read_to_string(store().join(format!("schema/{family}-v{version}.schema"))).expect("schema readable");
    let output = generate(&parse(&source).expect("schema valid"));
    let code = store().join(format!("src/generated/{family}_v{version}.rs"));
    let goldens = store().join(format!("golden/{family}"));
    let write = std::env::var_os("SKEIN_CODEGEN_WRITE").is_some_and(|value| value == "1");
    let again = "run SKEIN_CODEGEN_WRITE=1 cargo nextest run -p jig-store-codecs-world --test drift";
    if write {
        fs::write(&code, &output.rust).expect("write generated code");
        fs::create_dir_all(&goldens).expect("create golden directory");
    } else {
        assert_eq!(fs::read_to_string(&code).expect("generated code committed"), output.rust, "{again}");
    }
    for golden in output.goldens {
        let path = goldens.join(golden.name);
        if write {
            fs::write(&path, golden.bytes).expect("write golden bytes");
        } else {
            assert_eq!(fs::read(&path).expect("golden bytes committed"), golden.bytes, "{again}");
        }
    }
}

#[test]
fn the_notes_familys_code_and_goldens_have_no_drift() {
    check("notes", 1);
}

#[test]
fn the_component_familys_code_and_goldens_have_no_drift() {
    check("component", 3);
}

#[test]
fn the_accounts_familys_code_and_goldens_have_no_drift() {
    check("accounts", 2);
}

#[test]
fn the_tasks_familys_code_and_goldens_have_no_drift() {
    check("tasks", 9);
}

#[test]
fn the_people_familys_code_and_goldens_have_no_drift() {
    check("people", 13);
}

#[test]
fn the_core_familys_code_and_goldens_have_no_drift() {
    check("core", 6);
}

#[test]
fn the_test_connectors_code_and_goldens_have_no_drift() {
    let root = skein_scratch::paths::workspace().join("testing/jig-test-store");
    let source = fs::read_to_string(root.join("schema/test-connector-v2.schema")).expect("schema readable");
    let output = generate(&parse(&source).expect("schema valid"));
    let code = root.join("src/generated/v2.rs");
    let goldens = root.join("golden");
    let write = std::env::var_os("SKEIN_CODEGEN_WRITE").is_some_and(|value| value == "1");
    if write {
        fs::write(code, &output.rust).expect("write code");
        fs::create_dir_all(&goldens).expect("golden directory");
    } else {
        assert_eq!(fs::read_to_string(code).expect("code committed"), output.rust);
    }
    for golden in output.goldens {
        let path = goldens.join(golden.name);
        if write {
            fs::write(path, golden.bytes).expect("write golden");
        } else {
            assert_eq!(fs::read(path).expect("golden committed"), golden.bytes);
        }
    }
}

#[test]
fn the_settings_familys_code_and_goldens_have_no_drift() {
    check("settings", 1);
}

/// The layouts' fingerprints the store keeps (store.md 5.3), from the schemas.
fn layouts() -> String {
    let mut code = String::from(
        "//! Each jig family's layout fingerprint in this build's schemas (store.md 5.3).\n\
         //! Written and checked by `SKEIN_CODEGEN_WRITE=1 cargo nextest run -p jig-store-codecs-world --test drift`.\n\
         use crate::codec::component::Layouts;\n\
         \n\
         pub(crate) const LAYOUTS: Layouts = Layouts {\n",
    );
    for (field, family, version) in [
        ("component", "component", 3),
        ("core", "core", 6),
        ("tasks", "tasks", 9),
        ("people", "people", 13),
        ("notes", "notes", 1),
        ("accounts", "accounts", 2),
        ("settings", "settings", 1),
    ] {
        let source = fs::read(store().join(format!("schema/{family}-v{version}.schema"))).expect("schema readable");
        let digest = skein_codec::schema::parse(&source).expect("schema valid").fingerprints().family().digest();
        let bytes: Vec<String> = digest.iter().map(|byte| format!("0x{byte:02x}")).collect();
        writeln!(code, "    {field}: [{}],", bytes.join(", ")).expect("a String takes the line");
    }
    code.push_str("};\n");
    code
}

#[test]
fn the_stores_layouts_have_no_drift() {
    let path = store().join("src/generated/layouts.rs");
    if std::env::var_os("SKEIN_CODEGEN_WRITE").is_some_and(|value| value == "1") {
        fs::write(&path, layouts()).expect("write layouts");
    } else {
        assert_eq!(
            fs::read_to_string(&path).expect("layouts committed"),
            layouts(),
            "run SKEIN_CODEGEN_WRITE=1 cargo nextest run -p jig-store-codecs-world --test drift"
        );
    }
}

#[test]
fn the_secrets_familys_code_and_goldens_have_no_drift() {
    check("secrets", 1);
}
