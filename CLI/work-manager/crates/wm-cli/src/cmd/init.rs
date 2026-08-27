use std::process::ExitCode;
use std::str::FromStr;

use anyhow::{Result, bail};
use wm_core::config::{MANIFEST, Preset};

use crate::ui;

pub fn run(preset: Option<String>, name: Option<String>, force: bool) -> Result<ExitCode> {
    let cwd = std::env::current_dir()?;
    let manifest = cwd.join(MANIFEST);
    if manifest.exists() && !force {
        bail!("{} already exists (use --force to overwrite)", manifest.display());
    }

    let preset = match preset {
        Some(raw) => Preset::from_str(&raw).map_err(anyhow::Error::msg)?,
        None => Preset::detect(&cwd),
    };
    let name = name.unwrap_or_else(|| {
        cwd.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "app".to_string())
    });

    let config = preset.template(&name);
    config.save(&manifest)?;

    println!("{} wrote {}", ui::OK, manifest.display());
    println!();
    println!("Review the build, test, artifacts and run entries, then:");
    println!("  wm dev new my-feature   # stage 1: isolated worktree");
    println!("  wm build --switch       # stage 2: build, test, activate");
    println!("  wm rollback             # stage 3: back out");
    Ok(ExitCode::SUCCESS)
}
