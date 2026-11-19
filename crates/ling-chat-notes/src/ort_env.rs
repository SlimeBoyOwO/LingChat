//! ONNX Runtime 进程级运行环境。
//!
//! ort 自 2.0.0-rc.14 起要求显式构建 `Environment`，且同一进程只允许构建一次，之后所有
//! Session 都必须共用它。这里集中持有该环境：嵌入引擎、笔记检索、情绪分类、VAD 与本地
//! TTS 统一从这里取，避免各处各自构建导致「only one environment」冲突。
//!
//! 各平台统一走 ort 的 download-binaries 预编译包（Windows 为 pyke 内置 DirectML 的 dll，
//! 由 copy-dylibs 摆到 target），因此直接 `ort::init` 构建即可，无需像旧版 load-dynamic
//! 那样先定位 onnxruntime.dll。

use std::sync::{Mutex, OnceLock};

use anyhow::{Context, Result};
use ort::environment::Environment;

static ENVIRONMENT: OnceLock<Environment> = OnceLock::new();

/// 构建需要串行化：并发首次取用时只允许一个线程真正构建，其余等待后复用。
static BUILD_LOCK: Mutex<()> = Mutex::new(());

/// 取进程内的 ONNX Runtime 环境，必要时构建。
///
/// 构建失败不会被缓存，调用方下次取用时会重试。
pub fn environment() -> Result<&'static Environment> {
    if let Some(environment) = ENVIRONMENT.get() {
        return Ok(environment);
    }

    // 锁持有到函数结束，保证「检查-构建-写入」串行。
    let _guard = BUILD_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(environment) = ENVIRONMENT.get() {
        return Ok(environment);
    }

    let environment = ort::init().build().context("构建 ONNX Runtime 环境失败")?;
    Ok(ENVIRONMENT.get_or_init(|| environment))
}
