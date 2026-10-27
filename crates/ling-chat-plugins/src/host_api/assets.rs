//! 读素材：`read_data_file` 与路径沙箱。

use rustpython_vm::{PyObjectRef, PyResult, VirtualMachine};

use super::guard::current_read_allow;
use super::value_to_pyobject;

/// 单次读取上限。插件读素材基本是为了转 base64 发给对端（视频、长语音这类也要能过），
/// 给宽一点；注意峰值内存约是这个数的 2～3 倍（原字节 + base64 串 + Python str 拷贝），
/// 再往上就该换流式/分片通道了。
const MAX_READ_BYTES: u64 = 64 * 1024 * 1024;

/// 校验「相对 `data/` 的路径」字面是否合法：非空、相对、不含 `..` 与盘符。
///
/// manifest 的 `read` 声明与插件传入的请求路径共用这一份校验，别各写一套。
pub(crate) fn check_relative_data_path(rel: &str) -> Result<(), String> {
    use std::path::Component;

    if rel.is_empty() {
        return Err("路径为空".to_string());
    }
    let candidate = std::path::Path::new(rel);
    if candidate.is_absolute() {
        return Err("只接受相对 data/ 的路径".to_string());
    }
    for comp in candidate.components() {
        match comp {
            Component::Normal(_) | Component::CurDir => {},
            _ => return Err("路径里不能有 .. 或盘符".to_string()),
        }
    }
    Ok(())
}

/// 拼 IO 错误的文案。**不带真实路径**：`io::Error` 的 Display 里含宿主绝对路径，
/// 原文回给插件等于把 data 目录的位置交代出去，所以这里只用插件给的相对路径。
fn io_error_text(rel: &str, e: &std::io::Error) -> String {
    let reason = match e.kind() {
        std::io::ErrorKind::NotFound => "文件不存在",
        std::io::ErrorKind::PermissionDenied => "没有权限",
        _ => "IO 错误",
    };
    format!("读取失败：{rel}（{reason}）")
}

/// 把插件给的相对路径解析成 `base` 下的真实路径。
///
/// 规则（这是沙箱的一部分，别放宽）：
/// - 只接受相对路径：绝对路径、盘符、`..` 一律拒绝；
/// - 解析后必须仍在 `base` 内：软链接指到外面也会被这一步拦下；
/// - 还必须落在 `allow`（本插件 manifest 的 `read` 声明）里某个前缀之下。
///
/// 不复用 `utils::path::validate_path_in_base`：这里要拿回 canonical 路径供读取，
/// 且错误文案不能回带宿主绝对路径（那个辅助函数的报错里全是路径）。
///
/// 显式传 base 是为了能测：全局 `data_dir()` 只在 App 启动时初始化，单测里拿不到。
fn resolve_data_path_in(
    base: &std::path::Path,
    allow: &[String],
    rel: &str,
) -> Result<std::path::PathBuf, String> {
    let rel = rel.trim();
    check_relative_data_path(rel)?;

    let real = base
        .join(rel)
        .canonicalize()
        .map_err(|e| io_error_text(rel, &e))?;
    let base_real = base
        .canonicalize()
        .map_err(|_| "data 目录不可用".to_string())?;
    if !real.starts_with(&base_real) {
        return Err("越界：只能读 data/ 下的文件".to_string());
    }
    // 声明的前缀也要 canonicalize 后再比，免得软链接把声明目录指向 data/ 别处
    let declared = allow.iter().any(|prefix| {
        base.join(prefix.trim())
            .canonicalize()
            .is_ok_and(|p| real.starts_with(&p))
    });
    if !declared {
        return Err(format!(
            "未声明读取该路径：{rel}（需在本插件 manifest 的 read 中声明其所在目录）"
        ));
    }
    Ok(real)
}

/// 读 `data/` 下的文件并转 base64；失败返回 `{ok: false, error}`，不抛异常。
fn read_data_file_impl(rel: &str) -> serde_json::Value {
    read_data_file_impl_in(&ling_chat_main::api::data_dir(), &current_read_allow(), rel)
}

/// `read_data_file_impl` 的显式 base / 声明版本（测试用临时目录）。
fn read_data_file_impl_in(
    base: &std::path::Path,
    allow: &[String],
    rel: &str,
) -> serde_json::Value {
    use base64::Engine as _;

    let rel = rel.trim();
    let path = match resolve_data_path_in(base, allow, rel) {
        Ok(path) => path,
        Err(e) => return serde_json::json!({ "ok": false, "error": e }),
    };
    let meta = match std::fs::metadata(&path) {
        Ok(meta) => meta,
        Err(e) => return serde_json::json!({ "ok": false, "error": io_error_text(rel, &e) }),
    };
    if !meta.is_file() {
        return serde_json::json!({ "ok": false, "error": "不是普通文件" });
    }
    if meta.len() > MAX_READ_BYTES {
        return serde_json::json!({
            "ok": false,
            "error": format!("文件太大（{} 字节，上限 {MAX_READ_BYTES}）", meta.len()),
        });
    }
    match std::fs::read(&path) {
        Ok(bytes) => serde_json::json!({
            "ok": true,
            "size": bytes.len(),
            "base64": base64::engine::general_purpose::STANDARD.encode(&bytes),
        }),
        Err(e) => serde_json::json!({ "ok": false, "error": io_error_text(rel, &e) }),
    }
}

pub(super) fn read_data_file(path: String, vm: &VirtualMachine) -> PyResult<PyObjectRef> {
    Ok(value_to_pyobject(vm, &read_data_file_impl(&path)))
}
