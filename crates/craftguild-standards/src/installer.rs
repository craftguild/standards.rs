//! Deterministic POSIX shell generation; embedded templates travel with the engine.

use crate::{
    config::{self, Config},
    distribution::atomic_write,
    installer_config::InstallerTransport,
};
use craftguild_core::{paths, Result};
use std::path::Path;

const TEMPLATE: &str = include_str!("installer.sh");

pub fn script(command: &str, repository: &str, prefix: &str) -> String {
    render(command, repository, prefix, InstallerTransport::Curl)
}

fn render(command: &str, repository: &str, prefix: &str, transport: InstallerTransport) -> String {
    TEMPLATE
        .replace("@COMMAND@", command)
        .replace("@REPOSITORY@", repository)
        .replace("@PREFIX@", prefix)
        .replace(
            "@TRANSPORT@",
            match transport {
                InstallerTransport::Curl => "curl",
                InstallerTransport::GithubCli => "github-cli",
            },
        )
}

pub fn generate(root: &Path, config: &Config, selected: Option<&str>, check: bool) -> Result<()> {
    let mut declarations: Vec<_> = config
        .installers
        .iter()
        .flatten()
        .filter(|i| selected.is_none_or(|id| i.artifact == id))
        .collect();
    declarations.sort_by_key(|i| &i.output);
    if selected.is_some() && declarations.is_empty() {
        return Err("No installer declared for the selected artifact".into());
    }
    // Validate every destination before writing any script, including missing parents.
    for installer in &declarations {
        let mut path = root.to_owned();
        for part in paths::relative(&installer.output)?.components() {
            path.push(part);
            if path.is_symlink() {
                return Err("Symlink in installer output".into());
            }
        }
        if path.exists() && !path.is_file() {
            return Err("Installer output is not a regular file".into());
        }
    }
    for installer in declarations {
        let artifact = config.artifact(&installer.artifact)?;
        let group = config
            .releases
            .iter()
            .find(|g| g.id == installer.release_group)
            .ok_or("Unknown installer release group")?;
        let expected = render(
            &artifact.command,
            &installer.repository,
            &group.tag_prefix,
            installer.transport,
        );
        let path = root.join(&installer.output);
        if check {
            if std::fs::read(&path).ok().as_deref() != Some(expected.as_bytes()) {
                return Err(format!(
                    "Installer differs: {}; run installer to regenerate",
                    installer.output
                )
                .into());
            }
        } else {
            std::fs::create_dir_all(path.parent().ok_or("Installer has no parent")?)?;
            atomic_write(&path, expected.as_bytes())?;
        }
    }
    Ok(())
}

pub fn run(args: &[String]) -> Result<()> {
    if matches!(args, [flag] if flag == "--help" || flag == "-h") {
        println!("Usage: installer [--artifact ID] [--check]");
        return Ok(());
    }
    let mut selected = None;
    let mut check = false;
    let mut args = args.iter();
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--artifact" if selected.is_none() => {
                selected = Some(args.next().ok_or("Missing artifact")?.as_str())
            }
            "--check" if !check => check = true,
            _ => return Err("Invalid installer arguments; use --help".into()),
        }
    }
    let root = std::env::current_dir()?.canonicalize()?;
    generate(&root, &config::load(&root)?, selected, check)
}
