//! 单条目安全检查与解压炸弹防护测试。

use ling_chat_lib::utils::archive::{
    check_entry_safety, ArchiveError, MAX_COMPRESSION_RATIO, MAX_ENTRY_COUNT,
};

#[test]
fn safety_allows_large_single_file_without_absolute_limit() {
    let result = check_entry_safety(0, 2 * 1024 * 1024, 150 * 1024 * 1024);
    assert!(result.is_ok());
}

#[test]
fn safety_allows_large_total_without_absolute_limit() {
    for index in 0..100 {
        let result = check_entry_safety(index, 10 * 1024 * 1024, 10 * 1024 * 1024);
        assert!(result.is_ok(), "entry {index} should pass");
    }
}

#[test]
fn bomb_blocks_high_ratio() {
    // 100KB 压缩 -> 15MB 解压 -> 比 153 (> MAX_COMPRESSION_RATIO 100)
    assert!(MAX_COMPRESSION_RATIO == 100, "MAX_COMPRESSION_RATIO 应为 100");
    let r = check_entry_safety(0, 100 * 1024, 15 * 1024 * 1024);
    assert!(matches!(r, Err(ArchiveError::CompressionRatio { .. })));
}

#[test]
fn bomb_allows_normal_zip() {
    for i in 0..100 {
        let r = check_entry_safety(i, 1024 * 1024, 2 * 1024 * 1024);
        assert!(r.is_ok(), "entry {i} 应通过");
    }
}

#[test]
fn bomb_blocks_too_many_entries() {
    let r = check_entry_safety(MAX_ENTRY_COUNT, 0, 0);
    assert!(matches!(r, Err(ArchiveError::TooManyEntries(_))));
}
