//! Android 下载保活：下载期间启动前台服务，避免息屏后被系统冻结。
//!
//! Android 会在大面积息屏/切后台后挂起进程，长下载（模型动辄数百 MB）很容易
//! 被中断。这里通过 JNI 调用 Kotlin 侧的 `DownloadKeepAliveService`，以常驻
//! 通知换取前台优先级；桌面端为 no-op。
//!
//! 使用方式：在下载任务开始时创建 [`KeepAliveGuard`]，任务结束（含失败）时
//! 它随作用域自动释放。多任务并发下载用引用计数合并成一个前台服务。

/// 下载期间持有前台服务；离开作用域时释放。
///
/// 非 Android 平台是零成本空操作，因此调用点无需 `cfg` 分支。
pub struct KeepAliveGuard {
    _private: (),
}

impl KeepAliveGuard {
    /// 开始保活。`label` 用于通知文案（通常是模型名）。
    pub fn start(label: &str) -> Self {
        imp::start(label);
        Self { _private: () }
    }
}

impl Drop for KeepAliveGuard {
    fn drop(&mut self) {
        imp::stop();
    }
}

#[cfg(target_os = "android")]
mod imp {
    use std::mem::ManuallyDrop;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use jni::objects::{JClass, JObject, JValue};

    const SERVICE_CLASS: &str = "com.noiq.lingchat.DownloadKeepAliveService";

    /// 并发下载的持有者数量：从 0 变 1 时启动服务，回到 0 时停止。
    static HOLDERS: AtomicUsize = AtomicUsize::new(0);

    pub fn start(label: &str) {
        if HOLDERS.fetch_add(1, Ordering::SeqCst) != 0 {
            return;
        }
        if let Err(error) = call_service("start", Some(label)) {
            tracing::warn!("启动 Android 下载保活服务失败: {error}");
        }
    }

    pub fn stop() {
        // fetch_sub 返回旧值；只有最后一个持有者负责停止。
        if HOLDERS.fetch_sub(1, Ordering::SeqCst) != 1 {
            return;
        }
        if let Err(error) = call_service("stop", None) {
            tracing::warn!("停止 Android 下载保活服务失败: {error}");
        }
    }

    /// 调用 `DownloadKeepAliveService.start(context, label)` / `.stop(context)`。
    fn call_service(method: &str, label: Option<&str>) -> Result<(), String> {
        // Tao 在应用启动时初始化 ndk-context，且不会释放，因此这里可以安全获取。
        let ctx = ndk_context::android_context();
        let vm = unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) }
            .map_err(|error| format!("获取 JavaVM 失败: {error}"))?;
        let mut env = vm
            .attach_current_thread()
            .map_err(|error| format!("attach 线程失败: {error}"))?;

        let result = call_service_inner(&mut env, ctx.context(), method, label);
        if result.is_err() {
            // 清掉可能挂起的 Java 异常，避免影响同线程后续 JNI 调用。
            let _ = env.exception_clear();
        }
        result
    }

    fn call_service_inner(
        env: &mut jni::JNIEnv<'_>,
        raw_context: *mut std::ffi::c_void,
        method: &str,
        label: Option<&str>,
    ) -> Result<(), String> {
        // ndk-context 保存的是全局引用，不能交给 JObject 的 Drop 去 DeleteLocalRef；
        // 先转成本线程的局部引用，全局引用仍归 ndk-context 所有。
        let global_context =
            ManuallyDrop::new(unsafe { JObject::from_raw(raw_context as jni::sys::jobject) });
        let context = env
            .new_local_ref(&*global_context)
            .map_err(|error| format!("创建 Context 局部引用失败: {error}"))?;

        // 附加到 JVM 的普通线程没有类加载器上下文，`find_class` 找不到应用内类；
        // 用 Application Context 的 ClassLoader 显式加载。
        let loader = env
            .call_method(&context, "getClassLoader", "()Ljava/lang/ClassLoader;", &[])
            .and_then(|value| value.l())
            .map_err(|error| format!("获取 ClassLoader 失败: {error}"))?;
        let class_name = env
            .new_string(SERVICE_CLASS)
            .map_err(|error| format!("创建类名失败: {error}"))?;
        let class = env
            .call_method(
                &loader,
                "loadClass",
                "(Ljava/lang/String;)Ljava/lang/Class;",
                &[JValue::Object(&class_name)],
            )
            .and_then(|value| value.l())
            .map(JClass::from)
            .map_err(|error| format!("加载 {SERVICE_CLASS} 失败: {error}"))?;

        let signature = if label.is_some() {
            "(Landroid/content/Context;Ljava/lang/String;)V"
        } else {
            "(Landroid/content/Context;)V"
        };
        let result = if let Some(label) = label {
            let title = env
                .new_string(label)
                .map_err(|error| format!("创建通知文案失败: {error}"))?;
            env.call_static_method(
                &class,
                method,
                signature,
                &[JValue::Object(&context), JValue::Object(&title)],
            )
        } else {
            env.call_static_method(&class, method, signature, &[JValue::Object(&context)])
        };
        result.map_err(|error| format!("调用 DownloadKeepAliveService.{method} 失败: {error}"))?;

        Ok(())
    }
}

#[cfg(not(target_os = "android"))]
mod imp {
    pub fn start(_label: &str) {}
    pub fn stop() {}
}
