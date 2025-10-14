use crate::build::run_command;
use color_eyre::eyre::{eyre, Result};
use fs_err as fs;
use std::path::Path;
use std::process::Command;
use tracing::{info, instrument};

#[instrument(level = "trace", skip_all)]
pub fn execute(
    package_dir: &Path,
    release: bool,
    profile: Option<&str>,
    targets: Vec<String>,
    packages: Vec<String>,
    dry_run: bool,
    verbose: bool,
) -> Result<()> {
    let package_dir = fs::canonicalize(package_dir)?;

    if !package_dir.exists() {
        let error = format!("Directory {:?} doesnt exists.", package_dir);
        return Err(eyre!(error));
    }

    info!("Removing 'target' folder from {:?}...", package_dir);

    let mut args: Vec<String> = vec!["clean", "--target-dir", "target"]
        .iter()
        .map(|v| v.to_string())
        .collect();

    if release {
        args.push("--release".to_string());
    }

    if let Some(prof) = profile {
        args.push("--profile".to_string());
        args.push(prof.to_string());
    }

    for target in targets {
        args.push("--target".to_string());
        args.push(target);
    }

    for package in packages {
        args.push("--package".to_string());
        args.push(package);
    }

    if dry_run {
        args.push("--dry-run".to_string());
    }

    run_command(
        Command::new("cargo")
            .args(&args[..])
            .current_dir(&package_dir),
        verbose,
    )?;

    info!("Done removing 'target' folder from {:?}.", package_dir);
    Ok(())
}
