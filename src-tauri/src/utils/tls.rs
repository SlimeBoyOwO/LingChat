//! 统一的 TLS 客户端配置。
//!
//! reqwest 0.13 默认用 rustls-platform-verifier 验证系统证书，Android 上
//! 未显式初始化会 TLS panic（TTS 下载、创意工坊、屏幕分析等全部崩溃）。
//! 这里用 webpki-roots（内置 Mozilla CA）构造 rustls ClientConfig 注入
//! reqwest，全平台行为一致、无需任何初始化。
//!
//! 所有需要 HTTPS 请求的模块都应通过 [`build_tls_config`] 配置客户端，
//! 不要各自复制这份代码。

use std::sync::Arc;

/// 构造预配置的 rustls ClientConfig（webpki-roots + aws-lc-rs）。
///
/// 用法：
/// ```text
/// let tls = build_tls_config().expect("TLS 配置失败");
/// let client = reqwest::Client::builder()
///     .tls_backend_preconfigured(tls)
///     .build()
///     .expect("reqwest client 构建失败");
/// ```
pub fn build_tls_config() -> Result<rustls::ClientConfig, String> {
    let mut roots = rustls::RootCertStore::empty();
    roots.roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::aws_lc_rs::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .map_err(|e| format!("rustls 协议版本配置失败: {e}"))
    .map(|builder| builder.with_root_certificates(Arc::new(roots)).with_no_client_auth())
}
