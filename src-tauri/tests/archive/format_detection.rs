//! 压缩包魔数识别测试。

use ling_chat_lib::utils::archive::{detect_format, ArchiveFormat, SEVENZ_MAGIC, ZIP_MAGIC};

#[test]
fn detect_zip() {
    let tmp = std::env::temp_dir().join("test_detect.zip");
    std::fs::write(&tmp, ZIP_MAGIC).unwrap();
    assert_eq!(detect_format(&tmp).unwrap(), ArchiveFormat::Zip);
    std::fs::remove_file(&tmp).unwrap();
}

#[test]
fn detect_7z() {
    let tmp = std::env::temp_dir().join("test_detect.7z");
    std::fs::write(&tmp, SEVENZ_MAGIC).unwrap();
    assert_eq!(detect_format(&tmp).unwrap(), ArchiveFormat::SevenZ);
    std::fs::remove_file(&tmp).unwrap();
}

#[test]
fn detect_unknown() {
    let tmp = std::env::temp_dir().join("test_detect.bin");
    std::fs::write(&tmp, b"hello world").unwrap();
    assert!(detect_format(&tmp).is_err());
    std::fs::remove_file(&tmp).unwrap();
}
