//! Standards-owned build ordering, candidate compilation and verified publication.

use crate::{
    build_config::Stage,
    config::{self, Artifact, Kind},
    distribution,
    execution_context::{Context, Options},
    execution_drivers as drivers, execution_hooks as hooks,
    execution_process::Runner,
    execution_record::{artifact_hashes, Receipt, Record},
    execution_transaction::{Lock, Snapshot},
};
use craftguild_core::{paths, seal::Prepared, Result};
use std::path::Path;
use std::time::Duration;

pub fn run(args: &[String], compile: bool) -> Result<()> {
    if matches!(args, [flag] if flag == "--help" || flag == "-h") {
        println!("Usage: build|prepare [--artifact ID] [--variant NAME] [--target TRIPLE] [--profile debug|release] [--toolchain NAME] [--features LIST] [--no-default-features] [--target-dir PATH] [--cancel-file PATH] [--timeout SECONDS]");
        return Ok(());
    }
    crate::execution_process::initialize_signals()?;
    let root = std::env::current_dir()?.canonicalize()?;
    let config = config::load(&root)?;
    let mut options = Options::parse(args)?;
    options.standard_layout = config.rust_output_layout.is_some();
    let selected =
        crate::build_config::artifact_order(&config, options.get("artifact").as_deref())?;
    if options.get("target").is_some() && !selected.iter().any(|a| a.kind == Kind::Rust) {
        return Err(
            "--target selects a Rust target; configure frontend targets in craftguild.toml".into(),
        );
    }
    let _lock = Lock::acquire(&root)?;
    for artifact in selected {
        let context = Context::resolve(&root, artifact, &options)?;
        execute(
            &root,
            artifact,
            context,
            &options,
            compile,
            config.schema_version == 2,
        )?;
    }
    Ok(())
}

pub fn execute(
    root: &Path,
    artifact: &Artifact,
    context: Context,
    options: &Options,
    compile: bool,
    fresh_assets: bool,
) -> Result<Prepared> {
    let build = artifact.build.clone().unwrap_or_default();
    let ordered = build.hook_order()?;
    let mut steps = vec!["prerequisites".into()];
    for stage in [Stage::Prerequisite, Stage::Prepare] {
        steps.extend(
            ordered
                .iter()
                .filter(|h| h.stage == stage)
                .map(|h| format!("hook:{}", h.id)),
        );
    }
    steps.push("assets".into());
    steps.extend(
        ordered
            .iter()
            .filter(|h| h.stage == Stage::Evidence)
            .map(|h| format!("hook:{}", h.id)),
    );
    steps.push("seal".into());
    if compile {
        if artifact.kind == Kind::Rust {
            steps.push("compiler".into());
        }
        steps.extend(
            ordered
                .iter()
                .filter(|h| h.stage == Stage::Verify)
                .map(|h| format!("hook:{}", h.id)),
        );
        steps.push("verify-output".into());
    }
    steps.push("publish".into());
    let mut record = Record::create(root, context.clone(), steps)?;
    let run_id = record.run.clone();
    let prepared_path = record
        .directory
        .join("prepared.json")
        .strip_prefix(root)?
        .to_str()
        .ok_or("Non-UTF8 prepared path")?
        .to_owned();
    let mut runner = Runner {
        toolchain: context.toolchain.clone(),
        compiler_version: Some(context.compiler_version.clone()),
        env: context.env(&prepared_path, &record.run)?,
        log: Some(record.directory.join("process.jsonl")),
        cancel_file: options
            .get("cancel-file")
            .map(|s| crate::execution_context::absolute(root, &s))
            .transpose()?,
        timeout: options
            .get("timeout")
            .map(|s| s.parse::<u64>().map(Duration::from_secs))
            .transpose()?,
        ..Runner::default()
    };
    runner.env.extend(context.environment.clone());
    crate::build_settings::environment(&options.environment)?;
    runner.env.extend(options.environment.clone());
    if let Some(toolchain) = &context.toolchain {
        runner
            .env
            .insert("RUSTUP_TOOLCHAIN".into(), toolchain.clone());
    }
    let cache_key = paths::digest(&serde_json::to_vec(&(
        &context.artifact,
        &context.toolchain,
    ))?);
    let cargo_cache = root.join(".craftguild/cache/rust").join(cache_key);
    runner.env.insert(
        "CARGO_TARGET_DIR".into(),
        cargo_cache.to_string_lossy().into(),
    );
    let mut snapshot = Snapshot::create(&record.directory)?;
    let mut selected = context.artifact(artifact);
    hooks::seal_declarations(&mut selected);
    let candidates = drivers::candidates(artifact, &context, &cargo_cache)?;
    runner.env.insert(
        "CRAFTGUILD_CANDIDATES".into(),
        serde_json::to_string(&candidates)?,
    );
    runner.env.insert(
        "CRAFTGUILD_CANDIDATE".into(),
        candidates
            .first()
            .ok_or("No candidate output")?
            .to_string_lossy()
            .into(),
    );
    let result = (|| -> Result<Prepared> {
        let (compiler_host, controls) = record.step("prerequisites", &runner, || {
            let controls = hooks::Evidence::inputs(
                root,
                &[
                    "craftguild.toml".into(),
                    artifact.manifest.clone(),
                    artifact.lockfile.clone(),
                ],
            )?;
            let host = drivers::prerequisites(root, artifact, &context, &runner)?;
            controls.verify(root)?;
            Ok((host, controls))
        })?;
        record.evidence("$configuration", controls)?;
        hooks::stage(
            root,
            artifact,
            Stage::Prerequisite,
            &mut record,
            &mut runner,
            &mut snapshot,
        )?;
        hooks::stage(
            root,
            artifact,
            Stage::Prepare,
            &mut record,
            &mut runner,
            &mut snapshot,
        )?;
        record.step("assets", &runner, || {
            drivers::assets(root, artifact, &runner, &mut snapshot, fresh_assets)
        })?;
        hooks::stage(
            root,
            artifact,
            Stage::Evidence,
            &mut record,
            &mut runner,
            &mut snapshot,
        )?;
        let directory = record
            .directory
            .strip_prefix(root)?
            .to_str()
            .ok_or("Non-UTF8 run path")?
            .to_owned();
        record.environment(&runner.env)?;
        let evidence = record.evidence.clone();
        let prepared = record.step("seal", &runner, || {
            hooks::verify_evidence(root, &evidence)?;
            let prepared = distribution::prepare_with(
                root,
                &selected,
                &context.target,
                context.profile.name(),
                &directory,
                &runner,
                false,
            )?;
            hooks::verify_evidence(root, &evidence)?;
            Ok(prepared)
        })?;
        if compile {
            if artifact.kind == Kind::Rust {
                record.step("compiler", &runner, || {
                    drivers::compile(root, &selected, &context, &runner)
                })?;
            }
            hooks::stage(
                root,
                artifact,
                Stage::Verify,
                &mut record,
                &mut runner,
                &mut snapshot,
            )?;
            record.step("verify-output", &runner, || {
                drivers::verify(
                    root,
                    artifact,
                    &prepared,
                    &context,
                    &runner,
                    &candidates,
                    compiler_host.as_deref(),
                )
            })?;
        }
        record.step("publish", &runner, || {
            prepared.verify(root, &prepared.build, &prepared.features)?;
            if compile {
                for (candidate, output) in candidates.iter().zip(&context.outputs) {
                    if candidate != output {
                        snapshot.publish(candidate, output)?;
                    }
                }
            }
            let public = root.join(format!(".craftguild/{}", artifact.command));
            snapshot.write(
                &public.join("prepared.json"),
                &serde_json::to_vec_pretty(&prepared)?,
            )?;
            snapshot.write(
                &public.join("bom.json"),
                &std::fs::read(root.join(&prepared.sbom))?,
            )?;
            if compile {
                let receipt = Receipt {
                    schema_version: 2,
                    protocol: crate::execution_context::PROTOCOL.into(),
                    run: run_id.clone(),
                    context: context.clone(),
                    environment: runner.env.clone(),
                    prepared: prepared.clone(),
                    outputs: artifact_hashes(&context)?,
                };
                receipt.verify(root)?;
                snapshot.write(
                    &public.join("receipt.json"),
                    &serde_json::to_vec_pretty(&receipt)?,
                )?;
            }
            runner.check()
        })?;
        Ok(prepared)
    })();
    match result {
        Ok(prepared) => {
            snapshot.finish()?;
            record.finish(if compile { "succeeded" } else { "prepared" })?;
            eprintln!(
                "Prepared {} for {}: {}",
                artifact.id, context.target, prepared.sbom
            );
            Ok(prepared)
        }
        Err(error) => {
            if let Err(cleanup) = runner.rollback_safe() {
                record.finish("failed")?;
                return Err(format!("{error}; {cleanup}").into());
            }
            snapshot.restore()?;
            record.finish(if runner.cancelled() {
                "cancelled"
            } else {
                "failed"
            })?;
            Err(error)
        }
    }
}
