use crate::valid_id;
use std::path::Path;
/// Recover an interrupted rename before PluginManager reconciles state.json.
/// Only our journaled, non-symlink staging directories are considered.
pub fn recover_installs(root: &Path) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let dir = entry.path();
        if !entry
            .file_name()
            .to_string_lossy()
            .starts_with(".game-install-")
            || !entry
                .file_type()
                .is_ok_and(|t| t.is_dir() && !t.is_symlink())
        {
            continue;
        }
        let Ok(id) = std::fs::read_to_string(dir.join("install-id")) else {
            continue;
        };
        if !valid_id(&id) {
            continue;
        }
        let target = root.join(&id);
        let previous = dir.join("previous");
        if previous.exists() && !target.exists() {
            if let Err(error) = std::fs::rename(&previous, &target) {
                tracing::error!("DLC rollback retained at {}: {error}", dir.display());
                continue;
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
