//! Declarative standalone installers and the opt-in canonical Rust layout.

use crate::{
    config::{Artifact, Config, Kind},
    release::versions::Group,
};
use craftguild_core::{paths, Result};
use serde::Deserialize;
use std::collections::BTreeSet;
use std::path::Path;

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RustOutputLayout {
    Standard,
}

#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InstallerTransport {
    #[default]
    Curl,
    GithubCli,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Installer {
    pub artifact: String,
    pub repository: String,
    pub release_group: String,
    pub output: String,
    #[serde(default)]
    pub transport: InstallerTransport,
}

pub fn owns(group: &Group, artifact: &Artifact) -> bool {
    group.cargo.iter().any(|cargo| {
        cargo.packages.contains(&artifact.package)
            && cargo.lockfile == artifact.lockfile
            && (cargo.manifest == artifact.manifest
                || cargo.workspace
                    && Path::new(&artifact.manifest)
                        .starts_with(Path::new(&cargo.manifest).parent().unwrap_or(Path::new(""))))
    })
}

pub fn validate(config: &Config) -> Result<()> {
    if config.schema_version != 2
        && (config.rust_output_layout.is_some() || config.installers.is_some())
    {
        return Err("Rust output layout and installers require configuration schema 2".into());
    }
    if config.rust_output_layout.is_some() {
        for artifact in config.artifacts.iter().filter(|a| a.kind == Kind::Rust) {
            if let Some(build) = &artifact.build {
                if build.output.is_some()
                    || build.output_root.is_some()
                    || build
                        .targets
                        .iter()
                        .any(|s| s.output.is_some() || s.output_root.is_some())
                    || build
                        .variants
                        .iter()
                        .any(|s| s.output.is_some() || s.output_root.is_some())
                {
                    return Err(
                        "Standard Rust layout forbids output and output_root overrides".into(),
                    );
                }
            }
        }
    }
    let mut artifacts = BTreeSet::new();
    let mut outputs: BTreeSet<&str> = BTreeSet::new();
    for installer in config.installers.iter().flatten() {
        let artifact = config.artifact(&installer.artifact)?;
        let group = config
            .releases
            .iter()
            .find(|g| g.id == installer.release_group)
            .ok_or("Unknown installer release group")?;
        if artifact.kind != Kind::Rust
            || !crate::build_config::identifier(&artifact.command)
            || artifact.build.as_ref().is_some_and(|b| b.library.is_some())
            || !owns(group, artifact)
            || config.releases.iter().filter(|g| owns(g, artifact)).count() != 1
        {
            return Err(
                "Installer requires an executable Rust artifact owned by its release group".into(),
            );
        }
        let parts: Vec<_> = installer.repository.split('/').collect();
        if parts.len() != 2 || !repository_owner(parts[0]) || !repository_name(parts[1]) {
            return Err("Installer repository must be a GitHub owner/repo identifier".into());
        }
        paths::relative(&installer.output)?;
        crate::build_config::output_path(&installer.output, &artifact.command)?;
        if installer.output.bytes().any(|b| b.is_ascii_control())
            || ["craftguild.toml", &artifact.manifest, &artifact.lockfile]
                .contains(&installer.output.as_str())
            || !artifacts.insert(&installer.artifact)
            || outputs.iter().any(|p| {
                *p == installer.output
                    || p.starts_with(&format!("{}/", installer.output))
                    || installer.output.starts_with(&format!("{p}/"))
            })
        {
            return Err("Installer output/artifact ownership must be unique and safe".into());
        }
        outputs.insert(&installer.output);
    }
    Ok(())
}

fn repository_owner(owner: &str) -> bool {
    !owner.is_empty()
        && owner.len() <= 39
        && !owner.starts_with('-')
        && !owner.ends_with('-')
        && owner
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

fn repository_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 100
        && name != "."
        && name != ".."
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
}
