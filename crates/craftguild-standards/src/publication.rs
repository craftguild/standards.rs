//! Static public-source readiness. Anonymous retrieval remains a separate acceptance gate.

use crate::command;
use craftguild_core::{paths, Result};
use serde_json::Value;
use std::path::Path;

const COMMON: &[&str] = &["craftguild-cli", "craftguild-cli-build", "craftguild-core"];

pub fn check(
    root: &Path,
    repository: &str,
    manifest: Option<&str>,
    locks: &[String],
) -> Result<Vec<(String, String)>> {
    let root = root.canonicalize()?;
    let mut errors = vec![];
    if let Some(manifest) = manifest {
        let meta: Value = serde_json::from_slice(&command::capture(
            &root,
            &[
                "cargo".into(),
                "metadata".into(),
                "--locked".into(),
                "--no-deps".into(),
                "--format-version".into(),
                "1".into(),
                "--manifest-path".into(),
                manifest.into(),
            ],
        )?)?;
        for package in meta["packages"]
            .as_array()
            .ok_or("Missing publication package inventory")?
        {
            let name = package["name"].as_str().ok_or("Missing package name")?;
            for dependency in package["dependencies"]
                .as_array()
                .ok_or("Missing dependency inventory")?
            {
                let target = dependency["name"]
                    .as_str()
                    .ok_or("Missing dependency name")?;
                if let Some(path) = dependency["path"].as_str() {
                    if !Path::new(path).canonicalize()?.starts_with(&root) {
                        errors.push(("CG-PUB-001".into(),format!("Local dependency escapes the standalone repository: {name} -> {target}")));
                    }
                    if COMMON.contains(&target) && repository != "craftguild/standards.rs" {
                        errors.push(("CG-PUB-002".into(),format!("Common dependency needs a fixed public standards Git revision: {name} -> {target}")));
                    }
                }
                if let Some(source) = dependency["source"]
                    .as_str()
                    .filter(|s| s.starts_with("git+"))
                {
                    let valid = rust_git(source).is_some_and(|url| {
                        !COMMON.contains(&target)
                            || url.trim_end_matches('/').trim_end_matches(".git")
                                == "https://github.com/craftguild/standards.rs"
                    });
                    if !valid {
                        errors.push(("CG-PUB-002".into(),format!("Git dependency needs credential-free HTTPS and a full fixed rev: {name} -> {target}")));
                    }
                }
                if COMMON.contains(&target)
                    && dependency["source"]
                        .as_str()
                        .is_some_and(|s| !s.starts_with("git+"))
                {
                    errors.push(("CG-PUB-002".into(),format!("Common dependency must use the public standards Git source: {name} -> {target}")));
                }
            }
        }
    }
    for lockfile in locks {
        let path = paths::owned(&root, lockfile)?;
        let data: Value = serde_json::from_slice(&std::fs::read(&path)?)?;
        if data["lockfileVersion"] != 3 {
            return Err("Publication npm checks require package-lock v3".into());
        }
        for (name, package) in data["packages"]
            .as_object()
            .ok_or("Missing npm publication inventory")?
        {
            let source = package["resolved"].as_str();
            let valid = if package["link"] == true || source.is_some_and(|s| s.starts_with("file:"))
            {
                source.is_some_and(|s| {
                    path.parent()
                        .unwrap()
                        .join(s.strip_prefix("file:").unwrap_or(s))
                        .canonicalize()
                        .is_ok_and(|p| p.starts_with(&root))
                })
            } else if let Some(source) = source {
                if let Some(git) = source.strip_prefix("git+") {
                    git.rsplit_once('#')
                        .is_some_and(|(url, rev)| public_https(url) && full_hash(rev))
                } else {
                    public_https(source)
                        && package["integrity"].as_str().is_some_and(|s| !s.is_empty())
                }
            } else {
                !name.contains("node_modules/") || package["inBundle"] == true
            };
            if !valid {
                errors.push((
                    "CG-PUB-003".into(),
                    format!("{lockfile}: package {name} has no portable public source"),
                ));
            }
        }
    }
    Ok(errors)
}

fn rust_git(source: &str) -> Option<&str> {
    let (url, query) = source.strip_prefix("git+")?.split_once("?rev=")?;
    let revision = query.split('#').next()?;
    (public_https(url) && full_hash(revision)).then_some(url)
}

fn full_hash(value: &str) -> bool {
    matches!(value.len(), 40 | 64)
        && value.bytes().all(|b| b.is_ascii_hexdigit())
        && value.bytes().any(|b| b != b'0')
}

fn public_https(value: &str) -> bool {
    value
        .strip_prefix("https://")
        .and_then(|s| s.split('/').next())
        .is_some_and(|host| !host.is_empty() && !host.contains('@'))
        && !value.chars().any(|c| c.is_control() || c.is_whitespace())
        && !value.contains('?')
        && !value.contains('#')
}

#[cfg(test)]
#[path = "../tests/unit/publication.rs"]
mod tests;
