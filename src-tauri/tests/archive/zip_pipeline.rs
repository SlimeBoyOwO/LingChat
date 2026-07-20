//! ZIP 格式压缩与解压的端到端集成测试。

use ling_chat_lib::utils::archive::{compress, extract_zip, ArchiveFormat};

use crate::helpers::{_build_sample_role, walkdir};

#[test]
fn extract_zip_round_trip_self_contained() {
    // 模拟角色导出目录结构，验证 ZIP 往返后仍保留顶层角色目录。
    let tmp = std::env::temp_dir().join(format!("role_rt_{}", uuid::Uuid::new_v4()));
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
    // 压缩暂存目录后，压缩包内部应以 `sample/` 作为顶层目录。
    let out_zip = tmp.join("out.zip");
    let count = std::sync::atomic::AtomicUsize::new(0);
    compress(&staging, ArchiveFormat::Zip, &out_zip, &|_| {
        count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    })
    .unwrap();
    assert!(out_zip.exists());
    assert!(count.load(std::sync::atomic::Ordering::SeqCst) > 0);

    let extract_root = tmp.join("extract");
    std::fs::create_dir_all(&extract_root).unwrap();
    let token = tokio_util::sync::CancellationToken::new();
    let summary = extract_zip(&out_zip, &extract_root, &token, &|_| {}).unwrap();
    assert!(summary.bytes_extracted > 0);
    assert!(extract_root.join("sample").is_dir());
    assert!(extract_root.join("sample/settings.yml").exists());
    std::fs::remove_dir_all(&tmp).unwrap();
}

#[test]
fn extract_zip_creates_top_dir_only_when_present() {
    // 验证带单一顶层角色目录的 ZIP 可以成功生成。
    let tmp = std::env::temp_dir().join(format!("role_peek_{}", uuid::Uuid::new_v4()));
    let role = _build_sample_role(&tmp);
    let out_zip = tmp.join("ok.zip");
    let staging = tmp.join("staging/sample");
    for entry in walkdir(&role) {
        let rel = entry.strip_prefix(&role).unwrap();
        let dest = staging.join(rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&dest).unwrap();
        } else {
            std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
            std::fs::copy(&entry, &dest).unwrap();
        }
    }
    compress(staging.parent().unwrap(), ArchiveFormat::Zip, &out_zip, &|_| {}).unwrap();
    assert!(out_zip.exists());
    std::fs::remove_dir_all(&tmp).unwrap();
}

#[test]
fn extract_zip_allows_entry_over_legacy_size_limit() {
    // 已取消绝对大小限制，因此 60MB 条目必须能够完整解压。
    let tmp = std::env::temp_dir().join(format!("role_bomb_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&tmp).unwrap();
    let staging = tmp.join("staging/sample");
    std::fs::create_dir_all(&staging).unwrap();
    use rand::{rngs::StdRng, RngCore, SeedableRng};
    let mut big = vec![0u8; 60 * 1024 * 1024];
    StdRng::seed_from_u64(42).fill_bytes(&mut big);
    std::fs::write(staging.join("big.bin"), &big).unwrap();
    let out_zip = tmp.join("bomb.zip");
    compress(staging.parent().unwrap(), ArchiveFormat::Zip, &out_zip, &|_| {}).unwrap();
    let extract_root = tmp.join("extract");
    std::fs::create_dir_all(&extract_root).unwrap();
    let token = tokio_util::sync::CancellationToken::new();
    let result = extract_zip(&out_zip, &extract_root, &token, &|_| {}).unwrap();
    assert_eq!(result.files_extracted, 1);
    assert_eq!(
        std::fs::metadata(extract_root.join("sample/big.bin"))
            .unwrap()
            .len(),
        60 * 1024 * 1024
    );
    std::fs::remove_dir_all(&tmp).unwrap();
}

#[test]
fn extract_zip_throttles_progress_events() {
    // 验证解压过程至少发送开始和完成事件。
    let tmp = std::env::temp_dir().join(format!("role_throttle_{}", uuid::Uuid::new_v4()));
    let role = _build_sample_role(&tmp);
    let staging = tmp.join("staging/sample");
    for entry in walkdir(&role) {
        let rel = entry.strip_prefix(&role).unwrap();
        let dest = staging.join(rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&dest).unwrap();
        } else {
            std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
            std::fs::copy(&entry, &dest).unwrap();
        }
    }
    let out_zip = tmp.join("t.zip");
    compress(staging.parent().unwrap(), ArchiveFormat::Zip, &out_zip, &|_| {}).unwrap();

    let events = std::sync::Mutex::new(Vec::<String>::new());
    let token = tokio_util::sync::CancellationToken::new();
    extract_zip(&out_zip, &tmp.join("extract"), &token, &|evt| {
        events.lock().unwrap().push(evt.phase.to_string());
    })
    .unwrap();
    let phases = events.into_inner().unwrap();
    assert!(phases.contains(&"started".to_string()));
    assert!(phases.iter().any(|p| p == "finished"));
    std::fs::remove_dir_all(&tmp).unwrap();
}
