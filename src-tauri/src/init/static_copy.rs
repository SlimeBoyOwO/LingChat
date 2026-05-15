use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tauri::{App, Manager};

pub fn resolve_data_dir() -> PathBuf {
    if cfg!(debug_assertions) {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("data")
    } else {
        std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .join("data")
    }
}

fn resolve_resource_game_data(app: &App) -> Result<PathBuf> {
    if cfg!(debug_assertions) {
        Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources").join("game_data"))
    } else {
        app.path()
            .resource_dir()
            .map(|p| p.join("game_data"))
            .context("Failed to resolve resource directory")
    }
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if file_type.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

pub fn copy_game_data(app: &App) -> Result<()> {
    let source = resolve_resource_game_data(app)?;
    let data_dir = resolve_data_dir();
    let target = data_dir.join("game_data");

    if !source.exists() {
        log::warn!("Resource game_data not found at {:?}, skipping copy", source);
        return Ok(());
    }

    if !target.exists() {
        log::info!("First run: copying game_data to {:?}", target);
        copy_dir_recursive(&source, &target)
            .context("Failed to copy game_data directory")?;
    } else {
        for entry in fs::read_dir(&source)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                let sub_target = target.join(entry.file_name());
                if !sub_target.exists() {
                    log::info!("Backfilling missing subdirectory: {:?}", entry.file_name());
                    copy_dir_recursive(&entry.path(), &sub_target)?;
                }
            }
        }
    }

    Ok(())
}
