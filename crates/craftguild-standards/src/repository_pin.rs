//! Fixed release provenance and managed entry ownership, without acquisition.

use craftguild_core::{paths, Result};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::Path};

pub const ENTRY_PROTOCOL: &str = "craftguild-entry-1";
pub const BASELINE: &str = "0.1.0";
pub const REFERENCE: &str = "standards/engine.json";
pub const LAUNCHER: &str = include_str!("../../../templates/launcher.py");
pub const MAKE: &str = include_str!("../../../templates/make/common.mk");

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub version: String,
    pub revision: String,
    pub baseline: String,
    pub protocol: String,
    pub entry_protocol: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MachineIdentity {
    pub command: String,
    pub engine: Identity,
    pub target: String,
    pub clean: bool,
}

impl MachineIdentity {
    pub fn from_build(info: craftguild_cli::BuildInfo<'_>) -> Self {
        Self {
            command: info.command.into(),
            engine: Identity {
                version: info.version.into(),
                revision: info.commit.into(),
                baseline: BASELINE.into(),
                protocol: crate::execution_context::PROTOCOL.into(),
                entry_protocol: ENTRY_PROTOCOL.into(),
            },
            target: info.target.into(),
            clean: info.dirty == Some(false),
        }
    }

    pub fn matches(&self, reference: &Reference) -> Result<()> {
        if !self.clean
            || self.engine != reference.release.identity
            || !reference.release.hosts.contains_key(&self.target)
        {
            return Err("Engine identity differs from the fixed clean release reference".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Asset {
    pub url: String,
    pub sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostAssets {
    pub binary: Asset,
    pub sbom: Asset,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoleAssets {
    pub initializer: HostAssets,
    pub engine: HostAssets,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Delivery {
    Split(RoleAssets),
    Legacy(HostAssets),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Release {
    pub schema_version: u32,
    pub identity: Identity,
    pub hosts: BTreeMap<String, Delivery>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reference {
    pub schema_version: u32,
    pub release: Release,
    pub entries: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub managed: BTreeMap<String, String>,
}

fn hexadecimal(text: &str, length: usize) -> bool {
    text.len() == length
        && text
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

impl Release {
    pub fn validate(&self) -> Result<()> {
        let identity = &self.identity;
        super::release::versions::stable(&identity.version)?;
        if !matches!(self.schema_version, 2 | 3)
            || !hexadecimal(&identity.revision, 40)
            || identity.baseline != BASELINE
            || identity.protocol != crate::execution_context::PROTOCOL
            || identity.entry_protocol != ENTRY_PROTOCOL
            || self.hosts.is_empty()
        {
            return Err("Incompatible published release metadata".into());
        }
        for (host, assets) in &self.hosts {
            if ![
                "aarch64-apple-darwin",
                "x86_64-apple-darwin",
                "aarch64-unknown-linux-gnu",
                "x86_64-unknown-linux-gnu",
            ]
            .contains(&host.as_str())
            {
                return Err("Unsupported release host".into());
            }
            let roles: Vec<(&str, &HostAssets)> = match (self.schema_version, assets) {
                (2, Delivery::Legacy(assets)) => vec![("craftguild-standards", assets)],
                (3, Delivery::Split(roles)) => vec![
                    ("craftguild-standards", &roles.initializer),
                    ("craftguild-standards-engine", &roles.engine),
                ],
                _ => return Err("Release artifact roles differ from metadata schema".into()),
            };
            for (command, assets) in roles {
                let base = format!("https://github.com/craftguild/standards.rs/releases/download/v{}/{command}-v{}-{host}", identity.version, identity.version);
                for (asset, url) in [
                    (&assets.binary, base.clone()),
                    (&assets.sbom, format!("{base}.bom.json")),
                ] {
                    if asset.url != url || !hexadecimal(&asset.sha256, 64) {
                        return Err(
                            "Release asset requires its exact role, HTTPS URL and SHA-256".into(),
                        );
                    }
                }
            }
        }
        Ok(())
    }
}

impl Reference {
    pub fn new(release: Release) -> Self {
        Self {
            schema_version: 3,
            managed: crate::init_plan::managed_hashes(),
            release,
            entries: Self::entry_hashes(),
        }
    }

    pub fn entry_hashes() -> BTreeMap<String, String> {
        [
            ("launcher.py".into(), paths::digest(LAUNCHER.as_bytes())),
            ("make/common.mk".into(), paths::digest(MAKE.as_bytes())),
        ]
        .into()
    }

    pub fn validate(&self) -> Result<()> {
        self.release.validate()?;
        if !matches!(self.schema_version, 2 | 3)
            || self.schema_version != self.release.schema_version
            || self.entries.keys().map(String::as_str).collect::<Vec<_>>()
                != ["launcher.py", "make/common.mk"]
            || self.entries.values().any(|hash| !hexadecimal(hash, 64))
        {
            return Err("Incompatible fixed entry reference; explicit migration required".into());
        }
        let expected = [
            ".agents/skills/craftguild-standardize/SKILL.md",
            "AGENTS.md",
        ];
        if (self.schema_version == 2 && !self.managed.is_empty())
            || (self.schema_version == 3
                && (self.managed.keys().map(String::as_str).collect::<Vec<_>>() != expected
                    || self.managed.values().any(|h| !hexadecimal(h, 64))))
        {
            return Err("Invalid managed baseline ownership".into());
        }
        Ok(())
    }

    pub fn verify_entries(&self, root: &Path) -> Result<()> {
        self.validate()?;
        for (name, hash) in &self.entries {
            if paths::hash_file(root, &format!("standards/{name}"))? != *hash {
                return Err("Managed entry hash differs from its fixed reference".into());
            }
        }
        Ok(())
    }
}

pub fn load(root: &Path) -> Result<Reference> {
    let reference: Reference = serde_json::from_slice(&fs::read(paths::owned(root, REFERENCE)?)?)?;
    reference.validate()?;
    Ok(reference)
}
