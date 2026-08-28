use crate::ui;
use anyhow::{Result,
             bail};
use gm_core::config::{MANIFEST,
                      Preset};
use gm_store::{detect_preset,
               save_config};
use std::{process::ExitCode,
          str::FromStr};

pub fn run(preset: Option<String>, name: Option<String>, force: bool) -> Result<ExitCode> {
    let cwd = std::env::current_dir()?;
    let manifest = cwd.join(MANIFEST);
    if manifest.exists() && !force {
        bail!("{} already exists (use --force to overwrite)", manifest.display());
    }

    let preset = match preset {
        | Some(raw) => Preset::from_str(&raw).map_err(anyhow::Error::msg)?,
        | None => detect_preset(&cwd),
    };
    let name = name.unwrap_or_else(|| cwd.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "app".to_string()));

    let config = preset.template(&name);
    save_config(&config, &manifest)?;

    println!("{} wrote {}", ui::OK, manifest.display());
    println!();
    println!("Review the build, test, artifacts and run entries, then:");
    println!("  gm worktree create my-feature       # stage 1: isolated worktree");
    println!("  gm generation build --activate      # stage 2: build, test, activate");
    println!("  gm generation rollback              # stage 3: back out");
    Ok(ExitCode::SUCCESS)
}
