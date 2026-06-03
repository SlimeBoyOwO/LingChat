use std::path::PathBuf;
use std::sync::OnceLock;

use tauri::Manager;

static DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

/// 初始化 data 目录缓存（必须在 App 启动时调用一次）。
pub fn init_data_dir(app: &tauri::AppHandle) {
    let dir = resolve_data_dir(app);
    DATA_DIR.set(dir).expect("data_dir already initialized");
}

/// 获取已缓存的 data 目录（必须先调用 `init_data_dir`）。
pub fn get_data_dir() -> &'static PathBuf {
    DATA_DIR
        .get()
        .expect("data_dir not initialized — call init_data_dir first")
}

/// 解析 data 目录路径。
///
/// - 开发模式（debug）：项目根目录下的 `data/`
/// - 移动端（android/ios）：平台沙盒内的应用数据目录
/// - 桌面端（release portable）：exe 所在目录下的 `data/`
///
/// 所有可读写数据（数据库、game_data、存档等）都放在此目录下。
fn resolve_data_dir(app: &tauri::AppHandle) -> PathBuf {
    if cfg!(debug_assertions) {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("data")
    } else if cfg!(any(target_os = "android", target_os = "ios")) {
        // 移动端必须使用平台沙盒路径，current_exe() 指向 APK/IPA 内部只读路径
        app.path()
            .app_data_dir()
            .expect("failed to resolve app_data_dir on mobile")
    } else {
        // 桌面端 portable：data 目录放在 exe 旁边，用户可直接访问
        std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .join("data")
    }
}
