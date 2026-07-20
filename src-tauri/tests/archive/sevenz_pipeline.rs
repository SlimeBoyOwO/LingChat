//! 7z 格式压缩与解压的端到端集成测试。

use ling_chat_lib::utils::archive::{extract_sevenz, ArchiveFormat};

use std::path::Path;

use crate::helpers::{_build_sample_role, walkdir};

#[test]
fn sevenz_round_trip_self_contained() {
    // 验证 7z 压缩与解压可以完整保留角色顶层目录和必要文件。
    let tmp = std::env::temp_dir().join(format!("role_7z_{}", uuid::Uuid::new_v4()));
    let role = _build_sample_role(&tmp);
    let staging = tmp.join("staging");
    let wrapped = staging.join("sample");
    for entry in walkdir(&role) {
        let rel = entry.strip_prefix(&role).unwrap();
        let dest = wrapped.join(rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&dest).unwrap();
        } else {
            std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
            std::fs::copy(&entry, &dest).unwrap();
        }
    }
    let out_7z = tmp.join("out.7z");
    sevenz_rust2::compress_to_path(&staging, &out_7z).expect("compress ok");
    let extract_root = tmp.join("extract");
    std::fs::create_dir_all(&extract_root).unwrap();
    let token = tokio_util::sync::CancellationToken::new();
    let summary = extract_sevenz(&out_7z, &extract_root, &token, &|_| {}).unwrap();
    assert!(summary.bytes_extracted > 0);
    assert!(extract_root.join("sample").is_dir());
    assert!(extract_root.join("sample/settings.yml").exists());
    std::fs::remove_dir_all(&tmp).unwrap();
    // 强制引用 ArchiveFormat 以避免 unused_imports
    let _ = ArchiveFormat::SevenZ;
    // 强制引用 Path 避免 unused_imports
    let _: &Path = &tmp;
}
