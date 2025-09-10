use crate::build::run_command;
use color_eyre::{eyre::eyre, Result};
use fs_err as fs;
use std::path::Path;
use std::process::Command;
use tracing::{info, instrument};

#[instrument(level = "trace", skip_all)]
pub fn execute(package_dir: &Path) -> Result<()> {
    let package_dir = fs::canonicalize(package_dir)?;
    // Check if the directory exists
    if !package_dir.exists() {
        let error = format!("Directory {:?} doesnt exists.", package_dir,);
        return Err(eyre!(error));
    }
    info!("Removing 'target' folder from {:?}...", package_dir);

    let args: Vec<String> = vec!["clean", "--target-dir", "target"]
        .iter()
        .map(|v| v.to_string())
        .collect();

    run_command(
        Command::new("cargo")
            .args(&args[..])
            .current_dir(&package_dir),
        false,
    )?;

    info!("Done removing 'target' folder from {:?}.", package_dir);
    Ok(())
}
