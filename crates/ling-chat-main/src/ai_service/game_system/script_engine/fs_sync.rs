use anyhow::{Context, Result};
#[cfg(not(windows))]
use std::fs;
use std::path::Path;

#[cfg(windows)]
pub(crate) fn sync_directory(path: &Path) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_GENERIC_WRITE, FILE_SHARE_DELETE,
        FILE_SHARE_READ, FILE_SHARE_WRITE, FlushFileBuffers, OPEN_EXISTING,
    };
    use windows::core::PCWSTR;

    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    wide.push(0);
    let handle = unsafe {
        CreateFileW(
            PCWSTR(wide.as_ptr()),
            FILE_GENERIC_WRITE.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            None,
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            None,
        )
    }
    .with_context(|| format!("打开目录刷盘句柄失败: {}", path.display()))?;
    let flush_result = unsafe { FlushFileBuffers(handle) }
        .with_context(|| format!("刷新目录元数据失败: {}", path.display()));
    let close_result = unsafe { CloseHandle(handle) }
        .with_context(|| format!("关闭目录刷盘句柄失败: {}", path.display()));
    flush_result.and(close_result)
}

#[cfg(not(windows))]
pub(crate) fn sync_directory(path: &Path) -> Result<()> {
    fs::File::open(path)
        .with_context(|| format!("打开目录刷盘句柄失败: {}", path.display()))?
        .sync_all()
        .with_context(|| format!("刷新目录元数据失败: {}", path.display()))
}
