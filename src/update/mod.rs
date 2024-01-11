use std::process::Command;

use super::build::run_command;

pub fn execute(mut user_args: Vec<String>, branch: &str) -> anyhow::Result<()> {
    let mut args: Vec<String> = vec!["install",
        "--git", "https://github.com/uqbar-dao/necdev",
        "--branch", branch,
    ]
        .iter()
        .map(|v| v.to_string())
        .collect();
    args.append(&mut user_args);
    run_command(Command::new("cargo").args(&args[..]))?;
    Ok(())
}
