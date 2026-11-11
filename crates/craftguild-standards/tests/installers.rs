use craftguild_standards::{config::Config, installer};

const CONFIG: &str = r#"schema_version=2
standards_version="0.1.0"
[[artifacts]]
id="demo"
command="demo"
project="demo"
kind="rust"
manifest="Cargo.toml"
lockfile="Cargo.lock"
package="demo"
[[releases]]
id="demo"
initial_version="1.2.3"
tag_prefix="demo-v"
notes="docs/releases"
paths=["src"]
[[releases.cargo]]
manifest="Cargo.toml"
lockfile="Cargo.lock"
packages=["demo"]
[[installers]]
artifact="demo"
repository="owner/repo"
release_group="demo"
output="scripts/install.sh"
"#;

#[test]
fn strict_declarations_and_deterministic_drift_check() {
    for invalid in [
        CONFIG.replace("schema_version=2", "schema_version=1"),
        CONFIG.replace("artifact=\"demo\"", "artifact=\"missing\""),
        CONFIG.replace("release_group=\"demo\"", "release_group=\"missing\""),
        CONFIG.replace("packages=[\"demo\"]", "packages=[\"other\"]"),
        CONFIG.replace("repository=\"owner/repo\"", "repository=\"../repo\""),
        CONFIG.replace("repository=\"owner/repo\"", "repository=\"https://github.com/owner/repo\""),
        CONFIG.replace("output=\"scripts/install.sh\"", "output=\"../install.sh\""),
        CONFIG.replace("output=\"scripts/install.sh\"", "output=\".git/config\""),
        CONFIG.replace("output=\"scripts/install.sh\"", "output=\"Cargo.toml\""),
        format!("{CONFIG}\n[[installers]]\nartifact=\"demo\"\nrepository=\"owner/repo\"\nrelease_group=\"demo\"\noutput=\"second.sh\""),
        CONFIG.replace("[[releases]]", "[artifacts.build.library]\nname=\"demo\"\nformats=[\"rlib\"]\n[[releases]]"),
        CONFIG.replace("kind=\"rust\"", "kind=\"frontend\""),
    ] {
        let config: Config = toml::from_str(&invalid).unwrap();
        assert!(config.validate().is_err(),"accepted {invalid}");
    }
    let root = tempfile::tempdir().unwrap();
    let config: Config = toml::from_str(CONFIG).unwrap();
    config.validate().unwrap();
    assert!(installer::generate(root.path(), &config, None, true).is_err());
    installer::generate(root.path(), &config, None, false).unwrap();
    let path = root.path().join("scripts/install.sh");
    let first = std::fs::read(&path).unwrap();
    installer::generate(root.path(), &config, Some("demo"), false).unwrap();
    assert_eq!(first, std::fs::read(&path).unwrap());
    installer::generate(root.path(), &config, None, true).unwrap();
    std::fs::write(&path, "drift").unwrap();
    assert!(installer::generate(root.path(), &config, None, true).is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "drift");
    assert!(installer::generate(root.path(), &config, Some("missing"), false).is_err());
    #[cfg(unix)]
    {
        std::fs::remove_file(&path).unwrap();
        std::os::unix::fs::symlink(root.path().join("elsewhere"), &path).unwrap();
        assert!(installer::generate(root.path(), &config, None, false).is_err());
    }
}

#[test]
fn private_transport_is_explicit_and_rejects_unknown_authentication_modes() {
    let root = tempfile::tempdir().unwrap();
    let config: Config = toml::from_str(&format!("{CONFIG}transport=\"github-cli\"\n")).unwrap();
    config.validate().unwrap();
    installer::generate(root.path(), &config, None, false).unwrap();
    let path = root.path().join("scripts/install.sh");
    let script = std::fs::read_to_string(&path).unwrap();
    assert!(script.contains("transport='github-cli'"));
    assert!(script.contains("repository='owner/repo'"));
    installer::generate(root.path(), &config, None, true).unwrap();
    let public: Config = toml::from_str(CONFIG).unwrap();
    installer::generate(root.path(), &public, None, false).unwrap();
    assert!(std::fs::read_to_string(path)
        .unwrap()
        .contains("transport='curl'"));
    assert!(toml::from_str::<Config>(&format!("{CONFIG}transport=\"basic\"\n")).is_err());
}
