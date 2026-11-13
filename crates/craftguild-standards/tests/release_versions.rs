use craftguild_standards::release::versions::{self, Group};
use std::{fs, path::Path, process::Command};

fn write(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn run(root: &Path, program: &str, args: &[&str]) -> String {
    let output = Command::new(program)
        .args(args)
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn cargo(root: &Path, args: &[&str]) -> String {
    run(root, "cargo", args)
}

fn group(extra: serde_json::Value) -> Group {
    let mut value = serde_json::json!({
        "id": "demo", "initial_version": "0.1.0", "tag_prefix": "v",
        "notes": "docs/releases", "paths": ["src"]
    });
    value
        .as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    serde_json::from_value(value).unwrap()
}

fn cargo_fixture() -> (tempfile::TempDir, Group) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    write(
        root,
        "Cargo.toml",
        r#"[workspace]
members = ["api", "tool"]
resolver = "2"
[workspace.package]
version = "0.1.0"
"#,
    );
    for name in ["api", "tool"] {
        write(
            root,
            &format!("{name}/Cargo.toml"),
            &format!(
                r#"[package]
name = "demo-{name}"
version.workspace = true
edition = "2021"
"#
            ),
        );
        write(root, &format!("{name}/src/lib.rs"), "pub fn ready() {}\n");
    }
    write(
        root,
        "consumer/Cargo.toml",
        r#"[workspace]
[package]
name = "consumer"
version = "9.0.0"
edition = "2021"
[dependencies]
demo-api = { path = "../api" }
"#,
    );
    write(
        root,
        "consumer/src/lib.rs",
        "pub fn ready() { demo_api::ready(); }\n",
    );
    cargo(root, &["generate-lockfile", "--offline"]);
    cargo(
        root,
        &[
            "generate-lockfile",
            "--offline",
            "--manifest-path",
            "consumer/Cargo.toml",
        ],
    );
    let group = group(serde_json::json!({"cargo": [{
        "manifest": "Cargo.toml", "lockfile": "Cargo.lock", "workspace": true,
        "packages": ["demo-api", "demo-tool"],
        "additional_lockfiles": ["consumer/Cargo.lock"]
    }]}));
    (temp, group)
}

#[test]
fn additional_path_consumer_locks_remain_locked_after_version_preparation() {
    let (temp, group) = cargo_fixture();
    let root = temp.path();
    versions::validate(std::slice::from_ref(&group)).unwrap();
    let edits = versions::changes(root, &group, "0.1.0", "0.1.1").unwrap();
    assert_eq!(edits.len(), 3);
    for edit in edits {
        write(root, &edit.path, &edit.contents);
    }
    for manifest in ["Cargo.toml", "consumer/Cargo.toml"] {
        let metadata: serde_json::Value = serde_json::from_str(&cargo(
            root,
            &[
                "metadata",
                "--locked",
                "--offline",
                "--format-version=1",
                "--manifest-path",
                manifest,
            ],
        ))
        .unwrap();
        for package in metadata["packages"].as_array().unwrap() {
            assert_eq!(
                package["version"],
                if package["name"] == "consumer" {
                    "9.0.0"
                } else {
                    "0.1.1"
                }
            );
        }
    }
}

#[test]
fn lock_updates_preserve_external_sources_and_qualify_only_owned_references() {
    let (temp, mut group) = cargo_fixture();
    let root = temp.path();
    group.cargo[0].additional_lockfiles = vec!["alternate/Cargo.lock".into()];
    write(
        root,
        "alternate/Cargo.lock",
        r#"version = 3
[[package]]
name = "demo-api"
version = "0.1.0"
[[package]]
name = "demo-api"
version = "0.1.0"
source = "git+https://example.invalid/api?rev=1111111111111111111111111111111111111111#1111111111111111111111111111111111111111"
[[package]]
name = "demo-tool"
version = "0.1.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
[[package]]
name = "consumer"
version = "9.0.0"
dependencies = [
 "demo-api 0.1.0",
 "demo-api 0.1.0 (git+https://example.invalid/api?rev=1111111111111111111111111111111111111111)",
 "demo-tool 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)",
]
"#,
    );
    let edits = versions::changes(root, &group, "0.1.0", "0.1.1").unwrap();
    let edit = edits
        .iter()
        .find(|e| e.path == "alternate/Cargo.lock")
        .unwrap();
    let lock: toml::Value = toml::from_str(&edit.contents).unwrap();
    let packages = lock["package"].as_array().unwrap();
    assert_eq!(packages[0]["version"].as_str(), Some("0.1.1"));
    for package in &packages[1..3] {
        assert_eq!(package["version"].as_str(), Some("0.1.0"));
    }
    let dependencies = packages[3]["dependencies"].as_array().unwrap();
    assert_eq!(dependencies[0].as_str(), Some("demo-api 0.1.1"));
    assert!(dependencies[1]
        .as_str()
        .unwrap()
        .starts_with("demo-api 0.1.0 (git+"));
    assert!(dependencies[2]
        .as_str()
        .unwrap()
        .starts_with("demo-tool 0.1.0 (registry+"));
}

#[test]
fn additional_lock_ownership_rejects_duplicates_unsafe_paths_missing_packages_and_drift() {
    let (temp, mut group) = cargo_fixture();
    for path in ["Cargo.lock", "../outside/Cargo.lock"] {
        group.cargo[0].additional_lockfiles = vec![path.into()];
        assert!(versions::validate(std::slice::from_ref(&group)).is_err());
    }
    group.cargo[0].additional_lockfiles = vec!["consumer/Cargo.lock".into()];
    for contents in [
        "version = 3\n[[package]]\nname = 'consumer'\nversion = '9.0.0'\n",
        "version = 3\n[[package]]\nname = 'demo-api'\nversion = '0.2.0'\n",
        "version = 3\n[[package]]\nname = 'demo-api'\nversion = '0.1.0'\n[[package]]\nname = 'demo-api'\nversion = '0.1.0'\n",
    ] {
        write(temp.path(), "consumer/Cargo.lock", contents);
        assert!(versions::changes(temp.path(), &group, "0.1.0", "0.1.1").is_err());
        assert!(fs::read_to_string(temp.path().join("Cargo.toml")).unwrap().contains("0.1.0"));
    }
}

#[test]
fn absent_additional_lockfiles_keep_historical_group_receipt_hashes() {
    let original = r#"{"id":"demo","initial_version":"0.1.0","tag_prefix":"v","notes":"docs/releases","paths":["src"],"cargo":[{"manifest":"Cargo.toml","lockfile":"Cargo.lock","workspace":true,"packages":["demo"]}],"npm":[]}"#;
    let group: Group = serde_json::from_str(original).unwrap();
    assert_eq!(serde_json::to_string(&group).unwrap(), original);
}

#[test]
fn npm_release_preserves_nested_condition_order_and_real_commonjs_esm_resolution() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let manifest = r##"{
  "name": "demo", "version": "0.1.0", "type": "module",
  "exports": {".": {"types": "./index.d.ts", "node": {
    "import": "./index.mjs", "require": "./index.cjs", "default": "./fallback.mjs"
  }, "default": "./fallback.mjs"}},
  "imports": {"#entry": {"types": "./index.d.ts", "import": "./index.mjs", "require": "./index.cjs", "default": "./fallback.mjs"}},
  "dependencies": {"z-last": "9.0.0", "a-first": "1.0.0"}
}"##;
    write(root, "package.json", manifest);
    write(
        root,
        "package-lock.json",
        r#"{"name":"demo","version":"0.1.0","lockfileVersion":3,"packages":{"":{"name":"demo","version":"0.1.0"},"node_modules/z-last":{"version":"9.0.0"},"node_modules/a-first":{"version":"1.0.0"}}}"#,
    );
    write(root, "index.cjs", "module.exports = 'commonjs';\n");
    write(root, "index.mjs", "export default 'module';\n");
    write(root, "fallback.mjs", "export default 'fallback';\n");
    let commonjs = [
        "-e",
        "process.stdout.write(require('demo') + ':' + require('#entry'))",
    ];
    let esm = [
        "--input-type=module",
        "-e",
        "import a from 'demo'; import b from '#entry'; process.stdout.write(a + ':' + b)",
    ];
    assert_eq!(run(root, "node", &commonjs), "commonjs:commonjs");
    assert_eq!(run(root, "node", &esm), "module:module");
    let group = group(
        serde_json::json!({"npm": [{"manifest": "package.json", "lockfile": "package-lock.json"}]}),
    );
    let before: serde_json::Value = serde_json::from_str(manifest).unwrap();
    let lock_before: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(root.join("package-lock.json")).unwrap()).unwrap();
    for edit in versions::changes(root, &group, "0.1.0", "0.1.1").unwrap() {
        write(root, &edit.path, &edit.contents);
    }
    assert_eq!(run(root, "node", &commonjs), "commonjs:commonjs");
    assert_eq!(run(root, "node", &esm), "module:module");
    let mut after: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(root.join("package.json")).unwrap()).unwrap();
    assert_eq!(after["version"], "0.1.1");
    after["version"] = "0.1.0".into();
    assert_eq!(
        serde_json::to_string(&before).unwrap(),
        serde_json::to_string(&after).unwrap()
    );
    let mut lock_after: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(root.join("package-lock.json")).unwrap()).unwrap();
    assert_eq!(lock_after["version"], "0.1.1");
    assert_eq!(lock_after["packages"][""]["version"], "0.1.1");
    lock_after["version"] = "0.1.0".into();
    lock_after["packages"][""]["version"] = "0.1.0".into();
    assert_eq!(
        serde_json::to_string(&lock_before).unwrap(),
        serde_json::to_string(&lock_after).unwrap()
    );
}
