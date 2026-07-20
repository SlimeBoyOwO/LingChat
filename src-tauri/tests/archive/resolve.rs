//! 同名冲突策略 (Skip / Rename / Overwrite) 测试。

use ling_chat_lib::utils::archive::{resolve_target, ArchiveError, ConflictPolicy};

#[test]
fn resolve_target_skip() {
    let base = std::env::temp_dir().join(format!("test_resolve_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&base).unwrap();

    let r1 = resolve_target(&base, "Alice", ConflictPolicy::Skip).unwrap();
    assert_eq!(r1.action, "created");
    std::fs::create_dir_all(&r1.target).unwrap();

    assert!(matches!(
        resolve_target(&base, "Alice", ConflictPolicy::Skip),
        Err(ArchiveError::AlreadyExists(_))
    ));

    let r2 = resolve_target(&base, "Alice", ConflictPolicy::Rename).unwrap();
    assert_eq!(r2.action, "renamed");
    assert_eq!(r2.final_name, "Alice_2");

    let r3 = resolve_target(&base, "Alice", ConflictPolicy::Overwrite).unwrap();
    assert_eq!(r3.action, "overwritten");

    std::fs::remove_dir_all(&base).unwrap();
}
