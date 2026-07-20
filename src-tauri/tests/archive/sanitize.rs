//! 压缩包条目名清洗测试。

use ling_chat_lib::utils::archive::{sanitize_entry_name, ArchiveError};

#[test]
fn sanitize_blocks_traversal() {
    assert!(matches!(
        sanitize_entry_name("../../etc/passwd"),
        Err(ArchiveError::PathTraversal(_))
    ));
    assert!(matches!(
        sanitize_entry_name("/etc/passwd"),
        Err(ArchiveError::PathTraversal(_))
    ));
    assert!(matches!(
        sanitize_entry_name("C:\\Windows\\System32"),
        Err(ArchiveError::PathTraversal(_))
    ));
    assert!(matches!(
        sanitize_entry_name("\\\\server\\share"),
        Err(ArchiveError::PathTraversal(_))
    ));
}

#[test]
fn sanitize_blocks_macos_metadata() {
    assert!(matches!(
        sanitize_entry_name("__MACOSX/foo"),
        Err(ArchiveError::InvalidName(_))
    ));
    assert!(matches!(
        sanitize_entry_name("role/._background.png"),
        Err(ArchiveError::InvalidName(_))
    ));
    assert!(matches!(
        sanitize_entry_name("role/.DS_Store"),
        Err(ArchiveError::InvalidName(_))
    ));
}

#[test]
fn sanitize_clean_path() {
    assert_eq!(sanitize_entry_name("Alice/bg.png").unwrap(), "Alice/bg.png");
    assert_eq!(sanitize_entry_name("a:b*c?.txt").unwrap(), "a_b_c_.txt");
    assert!(sanitize_entry_name("").is_err());
}
