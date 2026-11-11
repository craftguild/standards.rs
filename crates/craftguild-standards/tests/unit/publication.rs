use super::*;
use serde_json::json;

#[test]
fn fixed_git_sources_reject_mutability_transport_and_credentials() {
    let hash = "1234567890123456789012345678901234567890";
    assert!(rust_git(&format!(
        "git+https://github.com/craftguild/standards.rs?rev={hash}"
    ))
    .is_some());
    for source in ["git+ssh://git@github.com/example/project?rev=main",
        "git+https://github.com/example/project?branch=main",
        "git+https://github.com/example/project?rev=short",
        "git+https://user@example.invalid/project?rev=1234567890123456789012345678901234567890",
        "git+https://example.invalid/project?credential=fixture?rev=1234567890123456789012345678901234567890"] {
        assert!(rust_git(source).is_none());
    }
}

#[test]
fn npm_public_sources_local_escape_and_missing_hashes_are_explicit() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("package-lock.json");
    let lock = json!({"lockfileVersion":3,"packages":{"":{},"node_modules/public":{
        "version":"1.0.0","resolved":"https://registry.npmjs.org/public/-/public-1.0.0.tgz","integrity":"sha512-fixture"}}});
    std::fs::write(&path, lock.to_string()).unwrap();
    let locks = vec!["package-lock.json".into()];
    assert!(check(temp.path(), "craftguild/product", None, &locks)
        .unwrap()
        .is_empty());
    for source in [
        "file:../not-part-of-clone",
        "git+ssh://git@example.invalid/repo#1234567890123456789012345678901234567890",
        "https://user@example.invalid/package.tgz",
    ] {
        let mut changed = lock.clone();
        changed["packages"]["node_modules/public"]["resolved"] = json!(source);
        std::fs::write(&path, changed.to_string()).unwrap();
        assert_eq!(
            check(temp.path(), "craftguild/product", None, &locks).unwrap()[0].0,
            "CG-PUB-003"
        );
    }
    let mut changed = lock;
    changed["packages"]["node_modules/public"]
        .as_object_mut()
        .unwrap()
        .remove("integrity");
    std::fs::write(&path, changed.to_string()).unwrap();
    assert!(!check(temp.path(), "craftguild/product", None, &locks)
        .unwrap()
        .is_empty());
}
