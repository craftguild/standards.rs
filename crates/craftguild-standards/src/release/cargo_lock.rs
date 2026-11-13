//! Synchronize only explicitly owned, source-free Cargo lock packages.

use super::{read, Change};
use craftguild_core::Result;
use std::{collections::BTreeSet, path::Path};
use toml_edit::{value, DocumentMut};

pub(super) fn change(
    root: &Path,
    path: &str,
    packages: &[String],
    current: &str,
    next: &str,
    complete: bool,
) -> Result<Change> {
    let mut lock = read(root, path)?.parse::<DocumentMut>()?;
    let entries = lock["package"]
        .as_array_of_tables_mut()
        .ok_or("Cargo lock packages missing")?;
    let mut seen = BTreeSet::new();
    for entry in entries.iter_mut() {
        let name = entry["name"]
            .as_str()
            .ok_or("Lock package name missing")?
            .to_owned();
        if packages.contains(&name) && !entry.contains_key("source") {
            if entry["version"].as_str() != Some(current) || !seen.insert(name) {
                return Err("Cargo lock version/ownership mismatch".into());
            }
            entry["version"] = value(next);
        }
    }
    if seen.is_empty() || (complete && seen != packages.iter().cloned().collect()) {
        return Err("Cargo lock lacks owned packages".into());
    }
    // Cargo qualifies ambiguous local dependencies with a name and version.
    // Source-qualified registry/Git references must keep their original versions.
    for entry in entries.iter_mut() {
        if let Some(dependencies) = entry.get_mut("dependencies") {
            for dependency in dependencies
                .as_array_mut()
                .ok_or("Cargo lock dependencies must be an array")?
                .iter_mut()
            {
                let text = dependency.as_str().ok_or("Invalid Cargo lock dependency")?;
                if let Some(name) = text.strip_suffix(&format!(" {current}")) {
                    if seen.contains(name) {
                        *dependency = format!("{name} {next}").into();
                    }
                }
            }
        }
    }
    Change::existing(root, path, lock.to_string())
}
