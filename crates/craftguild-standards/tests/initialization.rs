use craftguild_standards::{
    config,
    init_transaction::{self, Edit},
    initialization, prime,
    repository_pin::{
        self as pin, Asset, Delivery, HostAssets, Identity, MachineIdentity, Reference, Release,
        RoleAssets,
    },
};
use std::{collections::BTreeMap, fs, path::Path, process::Command};

fn fixture() -> (tempfile::TempDir, Reference, MachineIdentity) {
    let root = tempfile::tempdir().unwrap();
    let identity = Identity {
        version: "7.8.9".into(),
        revision: "a".repeat(40),
        baseline: pin::BASELINE.into(),
        protocol: "craftguild-execution-1".into(),
        entry_protocol: pin::ENTRY_PROTOCOL.into(),
    };
    let host = "aarch64-apple-darwin";
    let assets = |command: &str| {
        let base = format!("https://github.com/craftguild/standards.rs/releases/download/v7.8.9/{command}-v7.8.9-{host}");
        HostAssets {
            binary: Asset {
                url: base.clone(),
                sha256: "b".repeat(64),
            },
            sbom: Asset {
                url: format!("{base}.bom.json"),
                sha256: "c".repeat(64),
            },
        }
    };
    let release = Release {
        schema_version: 3,
        identity: identity.clone(),
        hosts: BTreeMap::from([(
            host.into(),
            Delivery::Split(RoleAssets {
                initializer: assets("craftguild-standards"),
                engine: assets("craftguild-standards-engine"),
            }),
        )]),
    };
    let reference = Reference::new(release);
    reference.validate().unwrap();
    let machine = MachineIdentity {
        command: "craftguild-standards-engine".into(),
        engine: identity,
        target: host.into(),
        clean: true,
    };
    (root, reference, machine)
}

fn initialize(root: &Path, reference: &Reference) {
    let edits =
        initialization::plan(root, Some(serde_json::to_vec_pretty(reference).unwrap())).unwrap();
    init_transaction::publish(root, &edits, |_| Ok(()), |_| Ok(())).unwrap();
}

fn files(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn visit(root: &Path, directory: &Path, found: &mut BTreeMap<String, Vec<u8>>) {
        for item in fs::read_dir(directory).unwrap() {
            let path = item.unwrap().path();
            if path.file_name().unwrap() == ".craftguild" {
                continue;
            }
            if path.is_dir() {
                visit(root, &path, found);
            } else {
                found.insert(
                    path.strip_prefix(root).unwrap().to_str().unwrap().into(),
                    fs::read(path).unwrap(),
                );
            }
        }
    }
    let mut found = BTreeMap::new();
    visit(root, root, &mut found);
    found
}

#[test]
fn fresh_init_is_valid_unfinished_and_identical_retry_does_not_rewrite_files() {
    let (root, reference, _) = fixture();
    initialize(root.path(), &reference);
    assert!(!root.path().join(".git").exists());
    let starter = fs::read_to_string(root.path().join("craftguild.toml")).unwrap();
    let config: config::Config = toml::from_str(&starter).unwrap();
    assert!(config.artifacts.is_empty());
    assert!(config.validate().is_err());
    assert_eq!(
        fs::read_to_string(root.path().join(".gitignore")).unwrap(),
        "/.craftguild/\n"
    );
    assert!(root
        .path()
        .join(".agents/skills/craftguild-standardize/SKILL.md")
        .is_file());
    reference.verify_entries(root.path()).unwrap();
    let before = files(root.path());
    let modified = fs::metadata(root.path().join(pin::REFERENCE))
        .unwrap()
        .modified()
        .unwrap();
    initialize(root.path(), &reference);
    assert_eq!(before, files(root.path()));
    assert_eq!(
        modified,
        fs::metadata(root.path().join(pin::REFERENCE))
            .unwrap()
            .modified()
            .unwrap()
    );
}

#[test]
fn existing_product_contracts_configuration_recipes_and_ignore_are_preserved() {
    let (root, reference, _) = fixture();
    fs::write(
        root.path().join("craftguild.toml"),
        "product config preserved even if malformed [",
    )
    .unwrap();
    fs::write(
        root.path().join("Makefile"),
        "build:\n\tmy-product-builder\n",
    )
    .unwrap();
    fs::write(root.path().join("AGENTS.md"), "Before\nProduct contracts\n").unwrap();
    fs::write(
        root.path().join(".gitignore"),
        "# Product rules\nother/\n/.craftguild/\n",
    )
    .unwrap();
    initialize(root.path(), &reference);
    let before = files(root.path());
    assert_eq!(
        fs::read_to_string(root.path().join("Makefile")).unwrap(),
        "build:\n\tmy-product-builder\n"
    );
    assert_eq!(
        fs::read_to_string(root.path().join("craftguild.toml")).unwrap(),
        "product config preserved even if malformed ["
    );
    let agents = fs::read_to_string(root.path().join("AGENTS.md")).unwrap();
    assert!(agents.contains("Before\n"));
    assert!(agents.ends_with("Product contracts\n"));
    assert_eq!(agents.matches("craftguild:common:start").count(), 1);
    initialize(root.path(), &reference);
    assert_eq!(before, files(root.path()));
    assert_eq!(
        fs::read_to_string(root.path().join(".gitignore")).unwrap(),
        "# Product rules\nother/\n/.craftguild/\n"
    );
}

#[test]
fn generated_agents_pass_the_actual_quality_gate_after_product_configuration() {
    let (root, reference, _) = fixture();
    assert!(Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(root.path())
        .status()
        .unwrap()
        .success());
    let contracts = "## Repository contracts\nPreserve the product behavior.\n";
    fs::write(root.path().join("AGENTS.md"), contracts).unwrap();
    initialize(root.path(), &reference);
    fs::create_dir(root.path().join("src")).unwrap();
    fs::write(root.path().join("src/main.rs"), "fn main() {}\n").unwrap();
    fs::write(
        root.path().join("Cargo.toml"),
        "[package]\nname=\"product\"\nversion=\"0.1.0\"\nedition=\"2021\"\n",
    )
    .unwrap();
    fs::write(
        root.path().join("Cargo.lock"),
        "version=3\n[[package]]\nname=\"product\"\nversion=\"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        root.path().join("craftguild.toml"),
        r#"schema_version=2
standards_version="0.1.0"
rust_output_layout="standard"
[[artifacts]]
id="product"
command="product"
project="product"
kind="rust"
manifest="Cargo.toml"
lockfile="Cargo.lock"
package="product"
[[sources]]
pattern="src/**"
role="implementation"
reason="Product implementation"
[[sources]]
pattern="standards/**"
role="vendored"
reason="Standards-owned generated entry"
[[sources]]
pattern="Makefile"
role="implementation"
reason="Shared entry wiring"
[[adapters]]
language="rust"
test=["cargo", "test", "--offline", "--locked"]
format=["cargo", "fmt", "--check"]
lint=["cargo", "clippy", "--offline", "--locked", "--", "-D", "warnings"]
"#,
    )
    .unwrap();
    for _ in 0..2 {
        let cfg = config::load(root.path()).unwrap();
        let report = craftguild_standards::quality::assess(root.path(), &cfg, None).unwrap();
        assert!(report.findings.is_empty(), "{:?}", report.findings);
        let before = files(root.path());
        assert!(fs::read_to_string(root.path().join("AGENTS.md"))
            .unwrap()
            .ends_with(contracts));
        initialize(root.path(), &reference);
        assert_eq!(before, files(root.path()));
    }
    // The gate remains strict: only removing the generated baseline must fail.
    let path = root.path().join("AGENTS.md");
    let instructions = fs::read_to_string(&path).unwrap();
    fs::write(path, instructions.replace(pin::BASELINE, "")).unwrap();
    let report = craftguild_standards::quality::assess(
        root.path(),
        &config::load(root.path()).unwrap(),
        None,
    )
    .unwrap();
    assert_eq!(report.findings.len(), 1);
    assert_eq!(report.findings[0].rule, "CG-AGENT-001");
}

#[test]
fn conflicts_symlinks_and_malformed_markers_refuse_before_publishing() {
    use std::os::unix::fs::symlink;
    let (root, reference, _) = fixture();
    let outside = tempfile::tempdir().unwrap();
    symlink(outside.path(), root.path().join("standards")).unwrap();
    assert!(initialization::plan(root.path(), None).is_err());
    assert!(fs::read_dir(outside.path()).unwrap().next().is_none());
    fs::remove_file(root.path().join("standards")).unwrap();
    fs::write(
        root.path().join("AGENTS.md"),
        "<!-- craftguild:common:end -->",
    )
    .unwrap();
    assert!(initialization::plan(root.path(), None).is_err());
    fs::remove_file(root.path().join("AGENTS.md")).unwrap();
    initialize(root.path(), &reference);
    fs::write(root.path().join("standards/launcher.py"), "conflict").unwrap();
    let before = files(root.path());
    assert!(initialization::plan(root.path(), None).is_err());
    assert_eq!(before, files(root.path()));
    for unsafe_path in ["../escape", "/absolute", "a/./alias", "a//alias"] {
        assert!(Edit::new(root.path(), unsafe_path, vec![], false).is_err());
    }
}

#[test]
fn interruption_rolls_back_only_owned_writes_and_refuses_concurrent_inputs() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("existing"), "before").unwrap();
    let edits = vec![
        Edit::new(root.path(), "existing", b"after".to_vec(), false).unwrap(),
        Edit::new(root.path(), "nested/new", b"new".to_vec(), true).unwrap(),
        Edit::new(root.path(), "reference", b"pin".to_vec(), true).unwrap(),
    ];
    for fail_at in 0..3 {
        let before = files(root.path());
        assert!(init_transaction::publish(
            root.path(),
            &edits,
            |_| Ok(()),
            |index| {
                if index == fail_at {
                    Err("simulated interruption".into())
                } else {
                    Ok(())
                }
            }
        )
        .is_err());
        assert_eq!(before, files(root.path()));
        assert!(!root.path().join("nested").exists());
    }
    let error = init_transaction::publish(
        root.path(),
        &edits,
        |_| Ok(()),
        |index| {
            if index == 1 {
                fs::write(root.path().join("reference"), "concurrent owner")?;
            }
            Ok(())
        },
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("concurrently"));
    assert_eq!(fs::read(root.path().join("existing")).unwrap(), b"before");
    assert_eq!(
        fs::read(root.path().join("reference")).unwrap(),
        b"concurrent owner"
    );
    fs::remove_file(root.path().join("reference")).unwrap();
    assert!(init_transaction::publish(
        root.path(),
        &edits,
        |_| Ok(()),
        |index| {
            if index == 1 {
                fs::write(root.path().join("existing"), "concurrent replacement")?;
                return Err("interrupted".into());
            }
            Ok(())
        }
    )
    .is_err());
    assert_eq!(
        fs::read(root.path().join("existing")).unwrap(),
        b"concurrent replacement"
    );
}

#[test]
fn lock_and_failed_acquisition_leave_generated_files_unpublished() {
    let (root, reference, _) = fixture();
    let edits =
        initialization::plan(root.path(), Some(serde_json::to_vec(&reference).unwrap())).unwrap();
    assert!(init_transaction::publish(
        root.path(),
        &edits,
        |_| Err("transport failed".into()),
        |_| Ok(())
    )
    .is_err());
    assert!(files(root.path()).is_empty());
    fs::write(root.path().join(".craftguild/init.lock"), "another owner").unwrap();
    assert!(init_transaction::publish(root.path(), &edits, |_| Ok(()), |_| Ok(())).is_err());
    assert_eq!(
        fs::read(root.path().join(".craftguild/init.lock")).unwrap(),
        b"another owner"
    );
    assert!(files(root.path()).is_empty());
}

#[test]
fn prime_reports_status_topics_and_matching_identity_without_product_execution() {
    let (root, reference, machine) = fixture();
    initialize(root.path(), &reference);
    for config in [
        None,
        Some("malformed ["),
        Some(include_str!("../../../templates/craftguild.toml")),
    ] {
        let path = root.path().join("craftguild.toml");
        match config {
            Some(text) => fs::write(&path, text).unwrap(),
            None => fs::remove_file(path).unwrap(),
        }
        let before = files(root.path());
        let mut output = Vec::new();
        prime::render(root.path(), &[], &machine, &mut output).unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("Not ready"));
        assert!(text.contains(&machine.engine.revision));
        assert!(!text.contains('\u{1b}'));
        assert_eq!(before, files(root.path()));
    }
    for (topic, _, _) in prime::TOPICS {
        let mut output = Vec::new();
        prime::render(
            root.path(),
            &["--topic".into(), (*topic).into()],
            &machine,
            &mut output,
        )
        .unwrap();
        assert!(!output.is_empty());
        assert!(!output.contains(&0x1b));
    }
    let mut output = Vec::new();
    prime::render(
        root.path(),
        &["--topic".into(), "standards".into()],
        &machine,
        &mut output,
    )
    .unwrap();
    assert!(String::from_utf8(output)
        .unwrap()
        .contains(&format!("blob/{}/spec/cli.md", machine.engine.revision)));
    let error = prime::render(
        root.path(),
        &["--topic".into(), "unknown".into()],
        &machine,
        &mut vec![],
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("Available: standards, cli"));
    let mut other = machine.clone();
    other.engine.version = "8.0.0".into();
    assert!(prime::render(root.path(), &[], &other, &mut vec![]).is_err());
    let mut incompatible = reference.clone();
    incompatible.release.identity.protocol = "future".into();
    assert!(incompatible.validate().is_err());
}

#[test]
fn failed_success_output_rolls_back_the_complete_generated_set() {
    let (root, reference, _) = fixture();
    let edits =
        initialization::plan(root.path(), Some(serde_json::to_vec(&reference).unwrap())).unwrap();
    assert!(init_transaction::publish_with_finish(
        root.path(),
        &edits,
        |_| Ok(()),
        |_| Ok(()),
        || Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "closed output").into())
    )
    .is_err());
    assert!(files(root.path()).is_empty());
}

#[test]
fn configured_prime_never_runs_declared_product_gates_or_hooks() {
    let (root, reference, machine) = fixture();
    initialize(root.path(), &reference);
    fs::write(
        root.path().join("craftguild.toml"),
        r#"schema_version=2
standards_version="0.1.0"
[[artifacts]]
id="product"
command="product"
project="product"
kind="rust"
manifest="Cargo.toml"
lockfile="Cargo.lock"
package="product"
[[commands]]
id="forbidden"
purpose="must never execute from context"
argv=["touch", "executed"]
phase="check"
[[operations]]
name="check"
artifact="product"
[[operations.hooks]]
id="forbidden"
stage="prepare"
argv=["touch", "executed"]
outputs=["executed"]
"#,
    )
    .unwrap();
    let before = files(root.path());
    let mut output = Vec::new();
    prime::render(root.path(), &[], &machine, &mut output).unwrap();
    assert!(String::from_utf8(output)
        .unwrap()
        .contains("Structurally configured"));
    assert!(!root.path().join("executed").exists());
    assert_eq!(before, files(root.path()));
}

#[test]
fn legacy_execution_descriptor_requires_migration_instead_of_a_second_authority() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir_all(root.path().join("standards/execution")).unwrap();
    fs::write(
        root.path().join("standards/execution/engine.json"),
        "legacy fixed reference",
    )
    .unwrap();
    let before = files(root.path());
    let error = initialization::plan(root.path(), None)
        .err()
        .unwrap()
        .to_string();
    assert!(error.contains("explicit migration"));
    assert_eq!(before, files(root.path()));
    assert!(!root.path().join(".craftguild").exists());
}

#[test]
fn manifest_based_adoption_is_rejected_before_acquisition_or_publication() {
    const CHILD: &str = "CRAFTGUILD_MANIFEST_REGRESSION_CHILD";
    if let Some(mode) = std::env::var_os(CHILD) {
        let (_, _, mut identity) = fixture();
        if mode == "other-installed-version" {
            identity.engine.version = "8.0.0".into();
        }
        let mut output = Vec::new();
        let error = initialization::run(&[], &identity, &mut output)
            .unwrap_err()
            .to_string();
        assert!(error.contains("standards/manifest.json"), "{error}");
        assert!(error.contains("explicit migration"), "{error}");
        assert!(output.is_empty());
        return;
    }
    use std::os::unix::fs::PermissionsExt;
    for existing_pin in [false, true] {
        let (root, reference, _) = fixture();
        if existing_pin {
            initialize(root.path(), &reference);
        }
        fs::create_dir_all(root.path().join("standards/execution/make")).unwrap();
        fs::write(root.path().join("standards/manifest.json"), r#"{"schema_version":1,"standards_version":"0.1.0","engine_version":"0.2.0","revision":"f34211b79ced9c2384ba97fd524ca973c147d299"}"#).unwrap();
        fs::write(
            root.path().join("standards/execution/make/common.mk"),
            "# legacy entry\n",
        )
        .unwrap();
        fs::write(
            root.path().join("Makefile"),
            "include standards/execution/make/common.mk\n",
        )
        .unwrap();
        let before = files(root.path());
        assert!(initialization::plan(root.path(), None).is_err());
        let tools = tempfile::tempdir().unwrap();
        let curl = tools.path().join("curl");
        fs::write(
            &curl,
            "#!/bin/sh\n: > \"$CRAFTGUILD_MANIFEST_TRANSPORT_MARKER\"\nexit 97\n",
        )
        .unwrap();
        fs::set_permissions(curl, fs::Permissions::from_mode(0o755)).unwrap();
        let marker = tools.path().join("transport-called");
        let path = std::env::join_paths(std::iter::once(tools.path().to_path_buf()).chain(
            std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()),
        ))
        .unwrap();
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "manifest_based_adoption_is_rejected_before_acquisition_or_publication",
                "--nocapture",
            ])
            .current_dir(root.path())
            .env(
                CHILD,
                if existing_pin {
                    "other-installed-version"
                } else {
                    "fresh"
                },
            )
            .env("PATH", path)
            .env("CRAFTGUILD_MANIFEST_TRANSPORT_MARKER", &marker)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            !marker.exists(),
            "init attempted release lookup/acquisition"
        );
        assert_eq!(before, files(root.path()));
        assert_eq!(root.path().join(pin::REFERENCE).exists(), existing_pin);
        assert!(!root.path().join(".craftguild/engine").exists());
        if !existing_pin {
            assert!(!root.path().join(".craftguild").exists());
        }
    }
}

#[test]
fn numeric_policy_covers_all_transitions_in_both_force_modes() {
    use craftguild_standards::init_policy::{select, Change};
    let (_, _, mut tool) = fixture();
    tool.engine.version = "0.10.0".into();
    for force in [false, true] {
        assert_eq!(select(&tool, None, force).unwrap(), Change::Initialize);
        assert_eq!(
            select(&tool, Some(&tool.engine), force).unwrap(),
            Change::Revalidate
        );
        let mut old = tool.engine.clone();
        old.version = "0.9.0".into();
        assert_eq!(select(&tool, Some(&old), force).unwrap(), Change::Upgrade);
        old.version = "0.11.0".into();
        let result = select(&tool, Some(&old), force);
        if force {
            assert_eq!(result.unwrap(), Change::Downgrade);
        } else {
            assert!(result.unwrap_err().to_string().contains("init --force"));
        }
        old.version = tool.engine.version.clone();
        old.revision = "d".repeat(40);
        let result = select(&tool, Some(&old), force);
        if force {
            assert_eq!(result.unwrap(), Change::Replace);
        } else {
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("same-version replacement"));
        }
    }
    tool.clean = false;
    assert!(select(&tool, None, true).is_err());
    tool.clean = true;
    for version in ["0.01.0", "0.10.0-rc.1", "0.10.0+local", "Latest"] {
        tool.engine.version = version.into();
        assert!(select(&tool, None, true).is_err());
    }
}

#[test]
fn update_preserves_old_pin_cache_and_consumer_edits_on_every_failed_publication() {
    let (root, old, _) = fixture();
    initialize(root.path(), &old);
    let old_cache = root.path().join(".craftguild/engine/old-valid");
    fs::create_dir_all(&old_cache).unwrap();
    fs::write(old_cache.join("engine"), "valid old binary").unwrap();
    for name in [
        "Cargo.toml",
        "Cargo.lock",
        "package.json",
        "package-lock.json",
    ] {
        fs::write(root.path().join(name), "product owned").unwrap();
    }
    let mut next = old.clone();
    next.release.identity.version = "8.0.0".into();
    // The plan's transport-independent ownership tests use valid-shaped release URLs.
    for delivery in next.release.hosts.values_mut() {
        let Delivery::Split(roles) = delivery else {
            unreachable!()
        };
        for pair in [&mut roles.initializer, &mut roles.engine] {
            pair.binary.url = pair.binary.url.replace("7.8.9", "8.0.0");
            pair.sbom.url = pair.sbom.url.replace("7.8.9", "8.0.0");
        }
    }
    let before = files(root.path());
    let edits =
        initialization::plan(root.path(), Some(serde_json::to_vec(&next).unwrap())).unwrap();
    assert_eq!(edits.last().unwrap().name, pin::REFERENCE);
    for fail_at in 0..edits.len() {
        assert!(init_transaction::publish(
            root.path(),
            &edits,
            |_| Ok(()),
            |i| {
                if i == fail_at {
                    Err("interrupted update".into())
                } else {
                    Ok(())
                }
            }
        )
        .is_err());
        assert_eq!(before, files(root.path()));
        assert_eq!(
            fs::read(old_cache.join("engine")).unwrap(),
            b"valid old binary"
        );
    }
    assert!(init_transaction::publish(
        root.path(),
        &edits,
        |_| Err("verification failed".into()),
        |_| Ok(())
    )
    .is_err());
    assert_eq!(before, files(root.path()));
    for name in [
        "standards/launcher.py",
        "standards/make/common.mk",
        ".agents/skills/craftguild-standardize/SKILL.md",
        "AGENTS.md",
    ] {
        let path = root.path().join(name);
        let original = fs::read(&path).unwrap();
        fs::write(&path, "edited by product").unwrap();
        assert!(
            initialization::plan(root.path(), Some(serde_json::to_vec(&next).unwrap())).is_err(),
            "{name}"
        );
        fs::write(path, original).unwrap();
    }
    init_transaction::publish(root.path(), &edits, |_| Ok(()), |_| Ok(())).unwrap();
    let mut expected = before;
    expected.insert(pin::REFERENCE.into(), serde_json::to_vec(&next).unwrap());
    assert_eq!(expected, files(root.path()));
}

#[test]
fn known_flat_pin_updates_all_managed_baselines_and_protects_product_contracts() {
    let (root, target, _) = fixture();
    let launcher = include_bytes!("fixtures/initialization-v0.4.0/launcher.py");
    let make = include_bytes!("fixtures/initialization-v0.4.0/common.mk");
    let skill = include_bytes!("fixtures/initialization-v0.4.0/local-skill.md");
    fs::create_dir_all(root.path().join("standards/make")).unwrap();
    fs::create_dir_all(root.path().join(".agents/skills/craftguild-standardize")).unwrap();
    fs::write(root.path().join("standards/launcher.py"), launcher).unwrap();
    fs::write(root.path().join("standards/make/common.mk"), make).unwrap();
    fs::write(
        root.path()
            .join(".agents/skills/craftguild-standardize/SKILL.md"),
        skill,
    )
    .unwrap();
    let old_block = "<!-- craftguild:common:start -->\nUse Craftguild standards 0.1.0 with the fixed engine in standards/engine.json.\nRun `make prime` (or `python3 standards/launcher.py prime`) before adoption.\nRead relevant topics with `make prime PRIME_ARGS=\"--topic NAME\"`.\nFollow the repo-local craftguild-standardize Skill. Run `make check`.\n<!-- craftguild:common:end -->";
    let original_agents = format!("Before\n{old_block}\nProduct contracts\n");
    fs::write(root.path().join("AGENTS.md"), &original_agents).unwrap();
    let mut old = target.clone();
    old.schema_version = 2;
    old.release.schema_version = 2;
    old.managed.clear();
    for delivery in old.release.hosts.values_mut() {
        let Delivery::Split(roles) = delivery else {
            unreachable!()
        };
        *delivery = Delivery::Legacy(roles.initializer.clone());
    }
    old.entries = [
        (
            "launcher.py".into(),
            craftguild_core::paths::digest(launcher),
        ),
        (
            "make/common.mk".into(),
            craftguild_core::paths::digest(make),
        ),
    ]
    .into();
    fs::write(
        root.path().join(pin::REFERENCE),
        serde_json::to_vec(&old).unwrap(),
    )
    .unwrap();
    let next = serde_json::to_vec(&target).unwrap();
    let edits = initialization::plan(root.path(), Some(next.clone())).unwrap();
    assert!(init_transaction::publish(
        root.path(),
        &edits,
        |_| Ok(()),
        |i| {
            if i == edits.len() - 1 {
                Err("before pin publication".into())
            } else {
                Ok(())
            }
        }
    )
    .is_err());
    assert_eq!(
        fs::read(root.path().join("standards/launcher.py")).unwrap(),
        launcher
    );
    assert_eq!(
        fs::read_to_string(root.path().join("AGENTS.md")).unwrap(),
        original_agents
    );
    fs::write(
        root.path().join("AGENTS.md"),
        original_agents.replace("before adoption", "before custom adoption"),
    )
    .unwrap();
    assert!(initialization::plan(root.path(), Some(next)).is_err());
    fs::write(root.path().join("AGENTS.md"), original_agents).unwrap();
    init_transaction::publish(root.path(), &edits, |_| Ok(()), |_| Ok(())).unwrap();
    target.verify_entries(root.path()).unwrap();
    let agents = fs::read_to_string(root.path().join("AGENTS.md")).unwrap();
    assert!(agents.starts_with("Before\n") && agents.ends_with("Product contracts\n"));
    assert!(agents.contains("init [--force]"));
}

#[test]
fn concurrent_acquisition_edits_and_lock_replacement_remain_owned_by_their_writer() {
    let (root, reference, _) = fixture();
    initialize(root.path(), &reference);
    let edits = initialization::plan(root.path(), None).unwrap();
    let agents = root.path().join("AGENTS.md");
    assert!(init_transaction::publish(
        root.path(),
        &edits,
        |_| {
            fs::write(&agents, "concurrent product instructions")?;
            Ok(())
        },
        |_| Ok(())
    )
    .is_err());
    assert_eq!(
        fs::read(&agents).unwrap(),
        b"concurrent product instructions"
    );
    assert!(root.path().join(pin::REFERENCE).is_file());
    let lock = init_transaction::Lock::acquire(root.path()).unwrap();
    let path = root.path().join(".craftguild/init.lock");
    fs::remove_file(&path).unwrap();
    fs::write(&path, "new lock owner").unwrap();
    drop(lock);
    assert_eq!(fs::read(path).unwrap(), b"new lock owner");
}
