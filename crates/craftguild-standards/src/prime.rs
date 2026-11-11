//! Read-only matching-version context, independent of product execution.

use crate::repository_pin::{self as pin, MachineIdentity};
use craftguild_core::Result;
use std::{io::Write, path::Path};

pub const TOPICS: &[(&str, &str, &str)] = &[
    (
        "standards",
        "spec/README.md",
        include_str!("../../../spec/README.md"),
    ),
    ("cli", "spec/cli.md", include_str!("../../../spec/cli.md")),
    (
        "distribution",
        "spec/distribution.md",
        include_str!("../../../spec/distribution.md"),
    ),
    (
        "quality",
        "spec/quality.md",
        include_str!("../../../spec/quality.md"),
    ),
    (
        "development",
        "spec/development.md",
        include_str!("../../../spec/development.md"),
    ),
    (
        "publication",
        "spec/publication.md",
        include_str!("../../../spec/publication.md"),
    ),
    (
        "releases",
        "spec/releases.md",
        include_str!("../../../spec/releases.md"),
    ),
    (
        "architecture",
        "docs/design/architecture.md",
        include_str!("../../../docs/design/architecture.md"),
    ),
    (
        "configuration",
        "docs/design/configuration.md",
        include_str!("../../../docs/design/configuration.md"),
    ),
    (
        "execution",
        "docs/design/orchestration.md",
        include_str!("../../../docs/design/orchestration.md"),
    ),
    (
        "initialization",
        "docs/design/initialization.md",
        include_str!("../../../docs/design/initialization.md"),
    ),
    (
        "adoption",
        "docs/guides/adoption.md",
        include_str!("../../../docs/guides/adoption.md"),
    ),
    (
        "workflow",
        "skills/craftguild-standardize/references/workflow.md",
        include_str!("../../../skills/craftguild-standardize/references/workflow.md"),
    ),
    (
        "installers",
        "docs/guides/installers.md",
        include_str!("../../../docs/guides/installers.md"),
    ),
];

/// Render embedded local references as full-revision source links.
pub fn links(text: &str, source: &str, revision: &str) -> String {
    let mut result = String::new();
    let mut remaining = text;
    while let Some((before, rest)) = remaining.split_once("](") {
        result.push_str(before);
        result.push_str("](");
        let Some((target, after)) = rest.split_once(')') else {
            result.push_str(rest);
            return result;
        };
        if target.contains("://") || target.starts_with(['#', '/']) || target.starts_with("mailto:")
        {
            result.push_str(target);
        } else {
            let mut parts: Vec<_> = source.split('/').collect();
            parts.pop();
            for part in target.split('/') {
                match part {
                    ".." => {
                        parts.pop();
                    }
                    "." => (),
                    _ => parts.push(part),
                }
            }
            result.push_str(&format!(
                "https://github.com/craftguild/standards.rs/blob/{revision}/{}",
                parts.join("/")
            ));
        }
        result.push(')');
        remaining = after;
    }
    result.push_str(remaining);
    result
}

pub fn render(
    root: &Path,
    args: &[String],
    identity: &MachineIdentity,
    output: &mut impl Write,
) -> Result<()> {
    let reference = pin::load(root)?;
    reference.verify_entries(root)?;
    identity.matches(&reference)?;
    let topics = TOPICS
        .iter()
        .map(|(name, _, _)| *name)
        .collect::<Vec<_>>()
        .join(", ");
    if let [flag, topic] = args {
        if flag == "--topic" {
            let (_, path, text) = TOPICS
                .iter()
                .find(|(name, _, _)| name == topic)
                .ok_or_else(|| format!("Unknown prime topic {topic}. Available: {topics}"))?;
            output.write_all(links(text, path, &identity.engine.revision).as_bytes())?;
            return Ok(());
        }
    }
    if !args.is_empty() {
        return Err("Usage: craftguild-standards-engine prime [--topic NAME]".into());
    }
    let status = match crate::config::load(root) {
        Ok(_) => "Structurally configured; conformance has not been checked.".into(),
        Err(error) => format!(
            "Not ready: {}. This is preparation status, not conformance success.",
            error.to_string().replace(['\u{1b}', '\r', '\n'], " ")
        ),
    };
    writeln!(output, "# Craftguild repository context\n\nBaseline: {}  \nEngine: {}  \nRevision: {}\n\n## Configuration\n\n{status}\n\n## Adoption and ownership\n\nRead the relevant topics, configure actual artifacts, source roles, adapters and product gates, then run make check and artifact acceptance. Initialization does not infer packages or complete adoption. Use standard Rust outputs: <target-dir>/<target-triple>/<debug|release>/<file>. Keep common-crate Git declarations at the reference's full revision.\n\nStandards owns common engine, font renderer, measurement, acquisition, output, seal, receipt, release, installer and package tests. Products retain SDK/ABI, generator ownership/reproduction, domain behavior and actual integration checks. Remove retired common-adapter tests after identifying their upstream owner; do not move them into product runtime crates or retain figlet-rs/vendor solely for standards tests. Add product tests only for a concrete uncovered connection risk. Generic measurement gaps belong upstream.\n\nOnly init and explicit make setup may acquire the engine. Ordinary operations and prime require the verified fixed cache. Prime is offline and read-only; it does not run product code or certify conformance. These guarantees apply to the supplied common entry.\n\n## Topics\n\n{topics}\n\nRead a topic with make prime PRIME_ARGS=\"--topic NAME\" or python3 standards/launcher.py prime --topic NAME. Details use matching embedded text and pinned source links.\n", identity.engine.baseline, identity.engine.version, identity.engine.revision)?;
    Ok(())
}
