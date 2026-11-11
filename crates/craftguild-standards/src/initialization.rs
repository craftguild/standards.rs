//! Shared initialization policy for the installed initializer and the full engine.
pub use crate::init_plan::plan;
use crate::{
    init_policy::{self, Change},
    init_transaction,
    repository_pin::{self as pin, MachineIdentity, Reference, Release},
};
use craftguild_core::Result;
use std::{io::Write, path::Path};

fn launch(root: &Path, entry: &Path, operation: &str) -> Result<Vec<u8>> {
    crate::command::capture(
        root,
        &[
            "python3".into(),
            entry.to_string_lossy().into(),
            operation.into(),
        ],
    )
}

pub fn run(args: &[String], identity: &MachineIdentity, output: &mut impl Write) -> Result<()> {
    if matches!(args, [flag] if flag == "--help" || flag == "-h") {
        writeln!(output, "Usage: {} init [--force]\nSelect this tool's exact published version/revision. --force permits downgrade or same-version revision replacement only.", identity.command)?;
        return Ok(());
    }
    let force = match args {
        [] => false,
        [flag] if flag == "--force" => true,
        _ => return Err("Usage: init [--force]".into()),
    };
    let root = std::env::current_dir()?.canonicalize()?;
    if root.parent().is_none()
        || std::env::var_os("HOME")
            .is_some_and(|home| Path::new(&home).canonicalize().ok().as_ref() == Some(&root))
    {
        return Err("Init requires a repository directory, not filesystem root or HOME".into());
    }
    crate::init_plan::refuse_legacy_references(&root)?;
    let existing = init_transaction::read(&root, pin::REFERENCE)?;
    let previous = existing.as_ref().map(|_| pin::load(&root)).transpose()
        .map_err(|e| format!("Existing fixed reference is incompatible: {e}. Explicit migration/adaptation required"))?;
    // Discover local ownership/path conflicts before suggesting a version-only override.
    let observed = plan(&root, None)?;
    let change = init_policy::select(
        identity,
        previous.as_ref().map(|p| &p.release.identity),
        force,
    )?;
    crate::execution_process::initialize_signals()?;
    let _lock = init_transaction::Lock::acquire(&root)?;
    init_transaction::recheck(&root, &observed)?;
    if init_transaction::read(&root, pin::REFERENCE)? != existing {
        return Err("Initialization pin changed concurrently".into());
    }
    let bytes = if change == Change::Revalidate {
        let reference = previous.as_ref().unwrap();
        identity.matches(reference)?;
        if reference.schema_version != 3
            || reference.entries != Reference::entry_hashes()
            || reference.managed != crate::init_plan::managed_hashes()
        {
            return Err("Exact identity has inconsistent managed implementation/role data; explicit migration required".into());
        }
        existing.clone().unwrap()
    } else {
        let url = format!("https://github.com/craftguild/standards.rs/releases/download/v{0}/craftguild-release-v{0}.json", identity.engine.version);
        let bytes = crate::command::capture(
            &root,
            &[
                "curl".into(),
                "-fsSL".into(),
                "--proto".into(),
                "=https".into(),
                "--proto-redir".into(),
                "=https".into(),
                url,
            ],
        )?;
        let release: Release = serde_json::from_slice(&bytes)?;
        release.validate()?;
        if release.schema_version != 3 {
            return Err("Selected release lacks separate initializer/engine artifact roles".into());
        }
        let reference = Reference::new(release);
        identity.matches(&reference)?;
        format!("{}\n", serde_json::to_string_pretty(&reference)?).into_bytes()
    };
    init_transaction::recheck(&root, &observed)?;
    if init_transaction::read(&root, pin::REFERENCE)? != existing {
        return Err("Initialization pin changed concurrently".into());
    }
    let edits = plan(&root, Some(bytes))?;
    init_transaction::publish_locked(
        &root,
        &edits,
        |stage| {
            launch(&root, &stage.join("standards/launcher.py"), "setup")?;
            Ok(())
        },
        |_| crate::execution_process::Runner::default().check(),
        || {
            let before = previous
                .as_ref()
                .map(|p| {
                    format!(
                        "{} @ {}",
                        p.release.identity.version, p.release.identity.revision
                    )
                })
                .unwrap_or_else(|| "absent".into());
            writeln!(output, "Initialized fixed Craftguild entry: {}: {} -> {} @ {}.\nRead make prime; finish product configuration, then make check and artifact acceptance.\nExisting Makefile recipes are preserved: include standards/make/common.mk and wire its targets where needed.\nManually align common-crate Git revisions with {}. Cargo/npm inputs are preserved; initialization does not certify product conformance.", change.label(), before, identity.engine.version, identity.engine.revision, identity.engine.revision)?;
            output.flush()?;
            Ok(())
        },
    )
}

pub fn prime(args: &[String], output: &mut impl Write) -> Result<()> {
    if matches!(args, [flag] if flag == "--help" || flag == "-h") {
        writeln!(
            output,
            "Usage: craftguild-standards-engine prime [--topic NAME]"
        )?;
        return Ok(());
    }
    let root = std::env::current_dir()?.canonicalize()?;
    let reference = pin::load(&root).map_err(|e| {
        format!("Fixed reference missing/invalid: {e}. Run init or explicitly adapt the reference")
    })?;
    reference.verify_entries(&root)?;
    let mut argv = vec![
        "python3".into(),
        root.join("standards/launcher.py").to_string_lossy().into(),
        "prime".into(),
    ];
    argv.extend_from_slice(args);
    output.write_all(&crate::command::capture(&root, &argv)?)?;
    Ok(())
}
