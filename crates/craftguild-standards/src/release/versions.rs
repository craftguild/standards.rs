//! Explicit version ownership and manifest/lockfile edits for Cargo and npm.

use super::files::Change;
use craftguild_core::{paths, Result};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, path::Path};
use toml_edit::{value, DocumentMut};

#[path = "cargo_lock.rs"]
mod cargo_lock;

/// Select the effective increment once, separately from breaking-change notes.
pub fn select(
    current: &str,
    inferred: super::conventional::Bump,
    initial: bool,
    mode: super::Mode,
) -> Result<(super::conventional::Bump, String)> {
    use super::{conventional::Bump, Mode};
    let mut next = stable(current)?;
    let bump = match mode {
        Mode::ExplicitMajor => Bump::Major,
        Mode::Normal if initial => Bump::None,
        Mode::Normal if next.major == 0 && inferred == Bump::Major => Bump::Minor,
        Mode::Normal => inferred,
    };
    match bump {
        Bump::Major => {
            next.major = next.major.checked_add(1).ok_or("Version overflow")?;
            next.minor = 0;
            next.patch = 0;
        }
        Bump::Minor => {
            next.minor = next.minor.checked_add(1).ok_or("Version overflow")?;
            next.patch = 0;
        }
        Bump::Patch => next.patch = next.patch.checked_add(1).ok_or("Version overflow")?,
        Bump::None => (),
    }
    Ok((bump, next.to_string()))
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Group {
    pub id: String,
    pub initial_version: String,
    pub tag_prefix: String,
    pub notes: String,
    pub paths: Vec<String>,
    #[serde(default)]
    pub cargo: Vec<Cargo>,
    #[serde(default)]
    pub npm: Vec<Npm>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Cargo {
    pub manifest: String,
    pub lockfile: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub additional_lockfiles: Vec<String>,
    #[serde(default)]
    pub workspace: bool,
    pub packages: Vec<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Npm {
    pub manifest: String,
    pub lockfile: String,
}

pub fn stable(text: &str) -> Result<semver::Version> {
    let v = semver::Version::parse(text)?;
    if !v.pre.is_empty() || !v.build.is_empty() {
        return Err("Automatic release supports stable SemVer versions only".into());
    }
    Ok(v)
}

pub fn validate(groups: &[Group]) -> Result<()> {
    let mut ids = BTreeSet::new();
    let mut owned = BTreeSet::new();
    let mut prefixes = BTreeSet::new();
    for g in groups {
        stable(&g.initial_version)?;
        if g.id.is_empty()
            || !g.id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
            || !ids.insert(&g.id)
            || g.tag_prefix.is_empty()
            || !g.tag_prefix.as_bytes()[0].is_ascii_alphanumeric()
            || !prefixes.insert(&g.tag_prefix)
            || !g
                .tag_prefix
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
            || g.paths.is_empty()
            || (g.cargo.is_empty() && g.npm.is_empty())
        {
            return Err(
                "Release groups require unique ids/prefixes and explicit ownership/path scope"
                    .into(),
            );
        }
        paths::relative(&g.notes)?;
        if g.notes.starts_with('.') {
            return Err("Release notes must be public repository paths".into());
        }
        for path in &g.paths {
            paths::relative(path)?;
        }
        for c in &g.cargo {
            if c.packages.is_empty()
                || c.packages.iter().collect::<BTreeSet<_>>().len() != c.packages.len()
            {
                return Err("Cargo release needs distinct owned package names".into());
            }
        }
        for p in g
            .cargo
            .iter()
            .flat_map(|c| {
                [&c.manifest, &c.lockfile]
                    .into_iter()
                    .chain(&c.additional_lockfiles)
            })
            .chain(g.npm.iter().flat_map(|n| [&n.manifest, &n.lockfile]))
        {
            paths::relative(p)?;
            if !owned.insert(p) {
                return Err("Release groups cannot share version/lockfile ownership".into());
            }
        }
    }
    Ok(())
}

fn read(root: &Path, path: &str) -> Result<String> {
    Ok(std::fs::read_to_string(paths::owned(root, path)?)?)
}
fn cargo_version<'a>(doc: &'a DocumentMut, c: &Cargo) -> Result<&'a str> {
    let item = if c.workspace {
        &doc["workspace"]["package"]["version"]
    } else {
        &doc["package"]["version"]
    };
    item.as_str()
        .ok_or_else(|| "Cargo version authority must be an explicit version".into())
}
pub fn current(root: &Path, g: &Group) -> Result<String> {
    let mut versions = Vec::new();
    for c in &g.cargo {
        let doc = read(root, &c.manifest)?.parse::<DocumentMut>()?;
        versions.push(cargo_version(&doc, c)?.to_owned());
    }
    for n in &g.npm {
        let doc: serde_json::Value = serde_json::from_str(&read(root, &n.manifest)?)?;
        versions.push(doc["version"].as_str().ok_or("npm version missing")?.into());
    }
    let first = versions.first().ok_or("No version authority")?;
    stable(first)?;
    if versions.iter().any(|v| v != first) {
        return Err("Release group versions disagree".into());
    }
    Ok(first.clone())
}

pub fn changes(root: &Path, g: &Group, current: &str, next: &str) -> Result<Vec<Change>> {
    let mut edits = Vec::new();
    for c in &g.cargo {
        let text = read(root, &c.manifest)?;
        let mut doc = text.parse::<DocumentMut>()?;
        if cargo_version(&doc, c)? != current {
            return Err("Cargo version drift".into());
        }
        // Cargo's own metadata defines workspace membership, including inherited fields.
        let raw = crate::command::capture(
            root,
            &[
                "cargo".into(),
                "metadata".into(),
                "--offline".into(),
                "--locked".into(),
                "--no-deps".into(),
                "--format-version=1".into(),
                "--manifest-path".into(),
                c.manifest.clone(),
            ],
        )?;
        let meta: serde_json::Value = serde_json::from_slice(&raw)?;
        let members = meta["workspace_members"]
            .as_array()
            .ok_or("Cargo workspace metadata missing")?;
        let packages: Vec<_> = meta["packages"]
            .as_array()
            .ok_or("Cargo packages missing")?
            .iter()
            .filter(|p| members.contains(&p["id"]))
            .collect();
        let names: BTreeSet<_> = packages.iter().filter_map(|p| p["name"].as_str()).collect();
        if names != c.packages.iter().map(String::as_str).collect() {
            return Err("Cargo release package ownership must exactly match its workspace".into());
        }
        for p in packages {
            if p["version"].as_str() != Some(current) {
                return Err("Cargo member version disagrees".into());
            }
            let manifest = Path::new(
                p["manifest_path"]
                    .as_str()
                    .ok_or("Missing member manifest")?,
            );
            let relative = manifest
                .strip_prefix(root.canonicalize()?)?
                .to_str()
                .ok_or("Non-UTF8 manifest")?;
            let member = read(root, relative)?.parse::<DocumentMut>()?;
            if c.workspace
                && member["package"]["version"]
                    .get("workspace")
                    .and_then(toml_edit::Item::as_bool)
                    != Some(true)
            {
                return Err("Workspace release members must inherit version".into());
            }
            if p["dependencies"]
                .as_array()
                .ok_or("Missing dependencies")?
                .iter()
                .any(|d| {
                    names.contains(d["name"].as_str().unwrap_or(""))
                        && d["req"].as_str() != Some("*")
                })
            {
                return Err("Version-constrained internal dependencies need explicit migration before release".into());
            }
        }
        if c.workspace {
            doc["workspace"]["package"]["version"] = value(next);
        } else {
            doc["package"]["version"] = value(next);
        }
        edits.push(Change::existing(root, &c.manifest, doc.to_string())?);
        for (path, complete) in std::iter::once((&c.lockfile, true))
            .chain(c.additional_lockfiles.iter().map(|p| (p, false)))
        {
            edits.push(cargo_lock::change(
                root,
                path,
                &c.packages,
                current,
                next,
                complete,
            )?);
        }
    }
    for n in &g.npm {
        let mut doc: serde_json::Value = serde_json::from_str(&read(root, &n.manifest)?)?;
        let mut lock: serde_json::Value = serde_json::from_str(&read(root, &n.lockfile)?)?;
        if doc["version"] != current
            || lock["version"] != current
            || lock["packages"][""]["version"] != current
            || doc["name"] != lock["name"]
            || doc["name"] != lock["packages"][""]["name"]
            || lock["lockfileVersion"] != 3
        {
            return Err("npm manifest/lock root ownership or version mismatch".into());
        }
        doc["version"] = next.into();
        lock["version"] = next.into();
        lock["packages"][""]["version"] = next.into();
        edits.push(Change::existing(
            root,
            &n.manifest,
            format!("{}\n", serde_json::to_string_pretty(&doc)?),
        )?);
        edits.push(Change::existing(
            root,
            &n.lockfile,
            format!("{}\n", serde_json::to_string_pretty(&lock)?),
        )?);
    }
    if current == next {
        edits.clear();
    }
    Ok(edits)
}
