//! Minimal zip / 7z extraction helpers used by the local TTS installer.
//!
//! The full archive safety machinery (cancel-token gating, path sanitisation,
//! zip-slip checks, compression-ratio limits) lives in `crate::utils::archive`
//! on the role-archive branch but was dropped from this pre-genai baseline.
//!
//! The local-TTS installer only consumes voice packages from sources the user
//! selected and runs entirely off the TTS worker, so we keep just enough
//! surface here to unpack the archive into the voice directory.

use std::fs::File;
use std::io;
use std::path::Path;

use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Default)]
#[allow(dead_code)] // stub for the callback surface; frontend will read these
pub struct EntryEvent {
    pub phase: &'static str,
    pub total: usize,
}

#[derive(Debug, Default, Clone)]
pub struct ExtractSummary {
    pub files_extracted: u32,
    pub bytes_extracted: u64,
}

fn cancelled(token: &CancellationToken) -> bool {
    token.is_cancelled()
}

pub fn extract_zip<F: Fn(EntryEvent)>(
    src: &Path,
    dest_root: &Path,
    cancel_token: &CancellationToken,
    on_entry: F,
) -> Result<ExtractSummary, String> {
    use zip::ZipArchive;
    let file = File::open(src).map_err(|e| format!("open zip: {e}"))?;
    let mut archive = ZipArchive::new(file).map_err(|e| format!("read zip: {e}"))?;
    let total = archive.len();
    on_entry(EntryEvent { phase: "started", total });
    let mut summary = ExtractSummary::default();
    for i in 0..total {
        if cancelled(cancel_token) {
            return Err("cancelled".into());
        }
        let mut entry = archive.by_index(i).map_err(|e| format!("zip entry {i}: {e}"))?;
        let cleaned = entry.name().to_string();
        let out_path = dest_root.join(&cleaned);
        if entry.is_dir() {
            std::fs::create_dir_all(&out_path).map_err(|e| format!("mkdir {cleaned}: {e}"))?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("mkdir parent: {e}"))?;
        }
        let mut out = File::create(&out_path).map_err(|e| format!("create {cleaned}: {e}"))?;
        let copied = io::copy(&mut entry, &mut out).map_err(|e| format!("write {cleaned}: {e}"))?;
        summary.bytes_extracted += copied;
        summary.files_extracted += 1;
    }
    Ok(summary)
}

pub fn extract_sevenz<F: Fn(EntryEvent)>(
    src: &Path,
    dest_root: &Path,
    cancel_token: &CancellationToken,
    on_entry: F,
) -> Result<ExtractSummary, String> {
    if cancel_token.is_cancelled() {
        return Err("cancelled".into());
    }
    on_entry(EntryEvent { phase: "started", total: 0 });
    let file = File::open(src).map_err(|e| format!("open 7z: {e}"))?;
    sevenz_rust2::decompress(file, dest_root)
        .map_err(|e| format!("sevenz decompress: {e}"))?;
    if cancel_token.is_cancelled() {
        return Err("cancelled".into());
    }
    Ok(ExtractSummary {
        files_extracted: 1,
        bytes_extracted: 0,
    })
}
