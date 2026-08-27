use gm_core::error::{Error,
                     IoContext,
                     Result};
use std::{collections::BTreeMap,
          path::Path,
          process::Command};

/// Run a shell stage with inherited stdio, so build output streams live.
///
/// Commands go through `sh -c` on purpose: the manifest holds whatever the
/// project's own toolchain needs (`cargo build`, `npm ci && npm run build`,
/// `make`), and the tool stays language-agnostic.
pub fn run_stage(stage: &str, cmd: &str, dir: &Path, env: &BTreeMap<String, String>) -> Result<()> {
    let status = Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .current_dir(dir)
        .envs(env)
        .status()
        .ctx(format!("spawning `{cmd}`"))?;

    if status.success() {
        Ok(())
    } else {
        Err(Error::StageFailed {
            stage: stage.to_string(),
            code: status.code().unwrap_or(-1),
        })
    }
}

/// Run a command and capture stdout, discarding stderr. Used for git queries.
pub fn capture(program: &str, args: &[&str], dir: &Path) -> Result<String> {
    let output = Command::new(program)
        .args(args)
        .current_dir(dir)
        .output()
        .ctx(format!("spawning `{program}`"))?;

    if !output.status.success() {
        return Err(Error::StageFailed {
            stage: format!("{program} {}", args.join(" ")),
            code: output.status.code().unwrap_or(-1),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
