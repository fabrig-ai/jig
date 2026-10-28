use super::fixtures;
use crate::{Path, Problem, Segment, What, clients::provider_key};
const PROVIDER: &[u8] = br#""provider":{"key":"secrets/provider.pem","clients":[{"id":"app","secret":"secrets/app","redirect_uri":"https://app.example.com/callback"}],"addresses":[3]},"link":true"#;
fn bytes() -> Box<[u8]> {
    let bytes = fixtures::with(fixtures::ENGINE_SMALL, br#""link": true"#, PROVIDER);
    fixtures::with(
        &bytes,
        br#"{"number": 2, "kind": "deployment"}"#,
        br#"{"number": 2, "kind": "deployment"},{"number":3,"kind":"application"}"#,
    )
}
fn provider_path() -> Path {
    fixtures::path(&[b"clients", b"provider"])
}
fn client_path() -> Path {
    provider_path().member(b"clients").child(Segment::Index(0))
}
fn refuses(from: &[u8], to: &[u8], path: Path, what: What) {
    let document = fixtures::with(&bytes(), from, to);
    let found = fixtures::engine(&document).unwrap_err();
    assert_eq!(found.problems.as_slice(), &[Problem { path, what }]);
}
#[test]
fn provider_is_optional_and_its_private_paths_are_data_paths() {
    assert!(fixtures::engine(fixtures::ENGINE_SMALL).unwrap().clients.provider.is_none());
    let null = fixtures::with(fixtures::ENGINE_SMALL, br#""link": true"#, br#""link":true,"provider":null"#);
    assert!(fixtures::engine(&null).unwrap().clients.provider.is_none());
    let bytes = bytes();
    let document = crate::document(&bytes, &crate::DocumentLimits { bytes: 8192, tokens: 1536, depth: 16 }).unwrap();
    let dirs = crate::Directories::given(b"/config", b"/data", b"/cache").unwrap();
    let engine = crate::engine(&document, 0, &dirs, crate::Defaults::Document).unwrap();
    let provider = engine.clients.provider.unwrap();
    assert_eq!(provider.key.as_ref(), b"/data/secrets/provider.pem");
    assert_eq!(provider.clients[0].secret.as_ref(), b"/data/secrets/app");
    assert_eq!(provider.addresses.as_ref(), &[3]);
}
#[test]
fn client_ids_are_visible_ascii_and_unique_and_there_is_at_least_one_client() {
    for (id, what) in [
        ("", What::Bound),
        ("a b", What::Form),
        ("a\\t", What::Form),
        ("a\\u007f", What::Form),
        ("\\u00e9", What::Form),
    ] {
        let id = json_text(b"id", id.as_bytes());
        refuses(br#""id":"app""#, &id, client_path().member(b"id"), what);
    }
    for size in [1, 255, 256] {
        let value = json_text(b"id", &repeated(if size == 255 { b'~' } else { b'!' }, size));
        let document = fixtures::with(&bytes(), br#""id":"app""#, &value);
        if size <= 255 {
            fixtures::engine(&document).expect("valid configuration");
        } else {
            assert_eq!(fixtures::engine(&document).unwrap_err().problems.get(0).unwrap().what, What::Bound);
        }
    }
    refuses(
        br#""clients":[{"id":"app","secret":"secrets/app","redirect_uri":"https://app.example.com/callback"}]"#,
        br#""clients":[]"#,
        provider_path().member(b"clients"),
        What::Bound,
    );
    refuses(br#""clients":[{"id":"app","secret":"secrets/app","redirect_uri":"https://app.example.com/callback"}]"#, br#""clients":[{"id":"app","secret":"a","redirect_uri":"https://app.example.com"},{"id":"app","secret":"b","redirect_uri":"https://app.example.com"}]"#, provider_path().member(b"clients").child(Segment::Index(1)).member(b"id"), What::Unique);
}
#[test]
fn provider_redirects_require_https_or_loopback_http_with_a_port_and_no_fragment() {
    for uri in [
        "https://app.example.com",
        "https://app.example.com:443/cb?x=1",
        "http://127.0.0.1:8080/cb",
        "http://[::1]:80/cb",
        "http://localhost:1/cb",
    ] {
        let value = json_text(b"redirect_uri", uri.as_bytes());
        fixtures::engine(&fixtures::with(&bytes(), br#""redirect_uri":"https://app.example.com/callback""#, &value))
            .expect("valid redirect URI");
    }
    for uri in [
        "http://app.example.com:80/cb",
        "http://localhost/cb",
        "http://127.0.0.1/cb",
        "http://[::1]/cb",
        "http://localhost:/cb",
        "http://localhost:0/cb",
        "http://localhost:65536/cb",
        "http://localhost:80.evil/cb",
        "http://localhost.evil:80/cb",
        "https://app.example.com/cb#frag",
        "https://",
        "https:///cb",
        "ftp://localhost:80/cb",
        "https://user@app.example.com/cb",
        "https://app.example.com/has space",
    ] {
        let value = json_text(b"redirect_uri", uri.as_bytes());
        refuses(
            br#""redirect_uri":"https://app.example.com/callback""#,
            &value,
            client_path().member(b"redirect_uri"),
            What::Form,
        );
    }
    for size in [2048_usize, 2049] {
        let uri = joined(&[b"https://app.example.com/", &repeated(b'a', size.checked_sub(24).unwrap())]);
        assert_eq!(uri.len(), size);
        let value = json_text(b"redirect_uri", &uri);
        let document = fixtures::with(&bytes(), br#""redirect_uri":"https://app.example.com/callback""#, &value);
        if size == 2048 {
            fixtures::engine(&document).expect("valid configuration");
        } else {
            assert_eq!(
                fixtures::engine(&document).unwrap_err().problems.get(0).unwrap(),
                &Problem { path: client_path().member(b"redirect_uri"), what: What::Bound }
            );
        }
    }
}
#[test]
fn address_providers_must_be_of_kind_application_and_paths_must_be_data_paths() {
    for value in [br#""addresses":[0]"#.as_slice(), br#""addresses":[1]"#, br#""addresses":[2]"#, br#""addresses":[4]"#]
    {
        refuses(
            br#""addresses":[3]"#,
            value,
            provider_path().member(b"addresses").child(Segment::Index(0)),
            What::Refers,
        );
    }
    refuses(
        br#""key":"secrets/provider.pem""#,
        br#""key":"../provider.pem""#,
        provider_path().member(b"key"),
        What::Relative,
    );
    refuses(br#""secret":"secrets/app""#, br#""secret":"../app""#, client_path().member(b"secret"), What::Relative);
}
#[test]
fn provider_limits_are_positive_named_overrides_with_profile_defaults() {
    for (profile, codes, tokens) in [(crate::Profile::Small, 16, 64), (crate::Profile::Standard, 256, 1024)] {
        let mut limits = crate::profile(profile);
        assert_eq!((limits.clients.provider_codes, limits.clients.provider_tokens), (codes, tokens));
        for name in [b"clients.provider_codes".as_slice(), b"clients.provider_tokens"] {
            assert_eq!(crate::overrides::apply(&mut limits, name, 0), Err(What::Override));
            crate::overrides::apply(&mut limits, name, 7).unwrap();
            assert_eq!(crate::overrides::apply(&mut limits, name, u64::MAX), Err(What::Override));
        }
        assert_eq!((limits.clients.provider_codes, limits.clients.provider_tokens), (7, 7));
        limits.clients.provider_codes = 0;
        assert!(!jig_protocol_engine_clients::valid(&limits.clients));
        limits.clients.provider_codes = 1;
        limits.clients.provider_tokens = 0;
        assert!(!jig_protocol_engine_clients::valid(&limits.clients));
    }
}
#[test]
fn provider_key_refuses_non_rsa_der_and_malformed_armor() {
    let arbitrary = b"this is arbitrary DER-shaped test input, not a key";
    let size =
        skein_lib::base64::encoded_len(skein_lib::base64::Alphabet::Standard, u32::try_from(arbitrary.len()).unwrap())
            .unwrap();
    let mut writer = skein_lib::Writer::new(usize::try_from(size).unwrap());
    skein_lib::base64::encode(skein_lib::base64::Alphabet::Standard, arbitrary, &mut writer).unwrap();
    let encoded = writer.finish();
    for eol in [b"\n".as_slice(), b"\r\n", b"\r"] {
        let pem = joined(&[b"-----BEGIN PRIVATE KEY-----", eol, &encoded, eol, b"-----END PRIVATE KEY-----", eol]);
        assert_eq!(provider_key(&pem).unwrap_err().path, provider_path().member(b"key"));
    }
    for pem in [
        b"-----BEGIN RSA PRIVATE KEY-----\nYWJj\n-----END RSA PRIVATE KEY-----\n".as_slice(),
        b"-----BEGIN ENCRYPTED PRIVATE KEY-----\nYWJj\n-----END ENCRYPTED PRIVATE KEY-----\n",
        b"-----BEGIN CERTIFICATE-----\nYWJj\n-----END CERTIFICATE-----\n",
        b"-----BEGIN PRIVATE KEY-----\n!!!!\n-----END PRIVATE KEY-----\n",
        b"-----BEGIN PRIVATE KEY-----\n\n-----END PRIVATE KEY-----\n",
        b"prefix\n-----BEGIN PRIVATE KEY-----\nYWJj\n-----END PRIVATE KEY-----\n",
        b"-----BEGIN PRIVATE KEY-----\nYWJj\n-----END PRIVATE KEY-----\nsuffix",
        b"-----BEGIN PRIVATE KEY-----\nYWJj-----END PRIVATE KEY-----\n",
    ] {
        let problem = provider_key(pem).unwrap_err();
        assert_eq!(problem.path, provider_path().member(b"key"));
        assert_eq!(
            problem.what,
            What::ProviderKey { generate: "openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:3072" }
        );
    }
}

fn joined(parts: &[&[u8]]) -> Box<[u8]> {
    let mut length = 0_usize;
    for part in parts {
        length = length.checked_add(part.len()).unwrap();
    }
    let mut writer = skein_lib::Writer::new(length);
    for part in parts {
        writer.put(part).unwrap();
    }
    writer.finish()
}
fn json_text(name: &[u8], value: &[u8]) -> Box<[u8]> {
    joined(&[b"\"", name, b"\":\"", value, b"\""])
}
fn repeated(byte: u8, count: usize) -> Box<[u8]> {
    let mut writer = skein_lib::Writer::new(count);
    for _ in 0..count {
        writer.put(&[byte]).unwrap();
    }
    writer.finish()
}

#[test]
#[expect(
    clippy::disallowed_types,
    clippy::disallowed_macros,
    clippy::print_stderr,
    reason = "ordinary fixture setup measures and reports only key-generation time as the task requires"
)]
fn provider_files_are_private_and_rsa_key_policy_is_checked() {
    use std::{fs, os::unix::fs::PermissionsExt, process::Command, time::Instant};
    let engine = fixtures::engine(&bytes()).unwrap();
    let files = crate::files(&engine);
    for (name, at) in [
        (b"secrets/provider.pem".as_slice(), provider_path().member(b"key")),
        (b"secrets/app".as_slice(), client_path().member(b"secret")),
    ] {
        let mut found = None;
        for file in &files {
            if file.path.as_ref() == name {
                found = Some(file);
                break;
            }
        }
        let file = found.expect("provider file enumerated");
        assert!(file.private);
        assert_eq!(file.at, at);
    }
    let scratch = tempfile::tempdir().expect("private key scratch");
    let key = scratch.path().join("provider.pem");
    for (bits, option) in [(3072_u32, "rsa_keygen_bits:3072"), (2048, "rsa_keygen_bits:2048")] {
        let started = Instant::now();
        let output = Command::new("openssl")
            .args(["genpkey", "-algorithm", "RSA", "-pkeyopt", option, "-out"])
            .arg(&key)
            .output()
            .expect("openssl installed");
        assert!(output.status.success(), "key generation failed");
        fs::set_permissions(&key, fs::Permissions::from_mode(0o600)).expect("private key");
        eprintln!("configuration RSA {bits} key generation: {} us", started.elapsed().as_micros());
        let pem = fs::read(&key).expect("key bytes");
        if bits == 3072 {
            let der = provider_key(&pem).expect("3072 PKCS#8 accepted");
            issuer_member_paths(&pem, scratch.path());
            assert_eq!(skein_tls::rs256_public(der.expose()).expect("RSA").modulus.len(), 384);
            let pkcs1 = scratch.path().join("pkcs1.pem");
            let output = Command::new("openssl")
                .args(["rsa", "-traditional", "-in"])
                .arg(&key)
                .arg("-out")
                .arg(&pkcs1)
                .output()
                .expect("openssl installed");
            assert!(output.status.success());
            fs::set_permissions(&pkcs1, fs::Permissions::from_mode(0o600)).expect("private key");
            assert_eq!(
                provider_key(&fs::read(pkcs1).expect("PKCS#1")).unwrap_err().path,
                provider_path().member(b"key")
            );
        } else {
            assert_eq!(
                provider_key(&pem).unwrap_err(),
                Problem {
                    path: provider_path().member(b"key"),
                    what: What::ProviderKey {
                        generate: "openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:3072"
                    }
                }
            );
        }
    }
}

fn issuer_urls() -> crate::clients::ProviderUrls {
    crate::clients::ProviderUrls {
        issuer: Box::from(b"https://app.example.com".as_slice()),
        public_origin: Box::from(b"https://app.example.com".as_slice()),
        authorization_endpoint: Box::from(b"https://app.example.com/authorize".as_slice()),
        token_endpoint: Box::from(b"https://app.example.com/token".as_slice()),
        userinfo_endpoint: Box::from(b"https://app.example.com/userinfo".as_slice()),
        jwks_uri: Box::from(b"https://app.example.com/keys".as_slice()),
    }
}
fn issuer_member_paths(pem: &[u8], scratch: &std::path::Path) {
    use crate::clients::provider_issuer;
    use skein_lib::Secret;
    use std::{fs, os::unix::fs::OpenOptionsExt, process::Command};
    let file = scratch.join("client.secret");
    fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&file).expect("private secret file");
    let output = Command::new("openssl")
        .args(["rand", "-hex", "-out"])
        .arg(&file)
        .arg("32")
        .output()
        .expect("openssl installed");
    assert!(output.status.success());
    let secret = Secret::new(fs::read(file).expect("client secret").into_boxed_slice());
    let mut engine = fixtures::engine(&bytes()).unwrap();
    let provider = engine.clients.provider.as_mut().expect("configured provider");
    let files = [(provider.key.clone(), Secret::new(Box::from(pem))), (provider.clients[0].secret.clone(), secret)];
    let limits = crate::profile(crate::Profile::Small).clients;
    let issuer = provider_issuer(provider, &files, issuer_urls(), &limits).expect("issuer built from private bytes");
    assert!(issuer.discovery().starts_with(b"{\"issuer\":\"https://app.example.com\""));
    assert_eq!(
        provider_issuer(provider, &[], issuer_urls(), &limits).unwrap_err(),
        Problem { path: provider_path().member(b"key"), what: What::Unreadable }
    );
    assert_eq!(
        provider_issuer(provider, &files[..1], issuer_urls(), &limits).unwrap_err(),
        Problem { path: client_path().member(b"secret"), what: What::Unreadable }
    );
    let empty =
        [(provider.key.clone(), files[0].1.clone()), (provider.clients[0].secret.clone(), Secret::new(Box::new([])))];
    assert_eq!(
        provider_issuer(provider, &empty, issuer_urls(), &limits).unwrap_err(),
        Problem { path: client_path().member(b"secret"), what: What::Bound }
    );
    provider.clients[0].redirect_uri = Box::from(b"https://app.example.com:0/callback".as_slice());
    assert_eq!(
        provider_issuer(provider, &files, issuer_urls(), &limits).unwrap_err(),
        Problem { path: client_path().member(b"redirect_uri"), what: What::Form }
    );
}
