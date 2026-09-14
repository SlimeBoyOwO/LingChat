package com.noiq.lingchat

import android.Manifest
import android.app.Activity
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.Environment
import android.provider.DocumentsContract
import android.provider.Settings
import android.widget.Toast
import androidx.core.content.FileProvider
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.Permission
import app.tauri.annotation.PermissionCallback
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.io.File

/**
 * 存储访问权限插件。
 *
 * Sherpa-ONNX 模型目录默认位于应用沙箱（Android/data/<package>/files/
 * sherpa_onnx_models），无需存储权限。本插件主要用于「打开模型目录」等场景，
 * 并保留了将模型目录放到外部存储根（如 `/sdcard/.lc-sherpa-onnx`）时所需的
 * 权限申请能力：
 * - API>=30（Android 11+）：引导用户进入系统「所有文件访问权限」设置页
 *   （MANAGE_EXTERNAL_STORAGE，无法用运行时对话框申请）。
 * - API<=29：运行时申请 WRITE/READ_EXTERNAL_STORAGE（需在 Manifest 声明，
 *   maxSdkVersion=29，见 AndroidManifest.xml）。
 *
 * 前端通过 Rust 命令 `check_sherpa_storage_permission` /
 * `request_sherpa_storage_permission`（tts_sherpa.rs）经 PluginHandle 调用，
 * 以及 `open_sherpa_onnx_model_manager`（tts_sherpa.rs）经 `openModelDir` 打开目录。
 */
@TauriPlugin(
    permissions = [
        Permission(
            strings = [Manifest.permission.WRITE_EXTERNAL_STORAGE],
            alias = "SHERPA_STORAGE_WRITE"
        ),
        Permission(
            strings = [Manifest.permission.READ_EXTERNAL_STORAGE],
            alias = "SHERPA_STORAGE_READ"
        )
    ]
)
class StoragePermissionPlugin(private val activity: Activity) : Plugin(activity) {

    /** 是否已持有读写 Sherpa 模型目录所需的存储权限。 */
    @Command
    fun checkPermission(invoke: Invoke) {
        invoke.resolve(JSObject().apply {
            put("granted", hasStoragePermission())
        })
    }

    /**
     * 请求存储权限。
     *
     * 返回 `{ granted: Boolean, prompted: Boolean }`：
     * - `granted`：当前是否已授权（API>=30 引导到设置页后通常需回到 App 重新检查）。
     * - `prompted`：本次是否弹出了系统授权对话框/设置页。
     */
    @Command
    fun requestPermission(invoke: Invoke) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            if (!Environment.isExternalStorageManager()) {
                openAllFilesAccessSettings()
            }
            invoke.resolve(JSObject().apply {
                put("granted", Environment.isExternalStorageManager())
                put("prompted", true)
            })
        } else {
            requestPermissionForAliases(
                arrayOf("SHERPA_STORAGE_WRITE", "SHERPA_STORAGE_READ"),
                invoke,
                "onLegacyStoragePermissionResult"
            )
        }
    }

    @PermissionCallback
    fun onLegacyStoragePermissionResult(invoke: Invoke) {
        invoke.resolve(JSObject().apply {
            put("granted", hasStoragePermission())
            put("prompted", true)
        })
    }

    /**
     * 在系统文件管理器中打开模型目录。
     *
     * 优先用 DocumentsContract（系统文件 / DocumentsUI 原生支持）打开到指定目录；
     * 若无人处理则回退 FileProvider content URI，再回退打开外部存储根目录；
     * 全部失败时把路径复制到剪贴板并提示用户（语义上视为成功，不报错）。
     */
    @Command
    fun openModelDir(invoke: Invoke) {
        @InvokeArg
        class Args {
            lateinit var path: String
        }
        val path = try {
            invoke.parseArgs(Args::class.java).path
        } catch (e: Exception) {
            invoke.reject("缺少 path 参数: ${e.message}")
            return
        }
        val dir = File(path)
        if (!dir.isDirectory && !dir.mkdirs()) {
            invoke.reject("无法创建模型目录: $path")
            return
        }
        openFolderInManager(dir)
        invoke.resolve(JSObject().apply { put("opened", true) })
    }

    private fun openFolderInManager(dir: File) {
        // 1) 系统 DocumentsUI / 外部存储 DocumentsProvider（Android 5.0+ 原生支持）
        val base = Environment.getExternalStorageDirectory()
        val canonicalBase = base.absolutePath
        val canonicalDir = dir.absolutePath
        if (canonicalDir.startsWith(canonicalBase)) {
            val rel = canonicalDir.removePrefix(canonicalBase).removePrefix("/")
            val uri = DocumentsContract.buildDocumentUri(
                "com.android.externalstorage.documents",
                "primary:$rel"
            )
            val intent = Intent(Intent.ACTION_VIEW).apply {
                setDataAndType(uri, DocumentsContract.Document.MIME_TYPE_DIR)
                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            }
            if (canResolve(intent)) {
                activity.startActivity(intent)
                return
            }
        }
        // 2) FileProvider content URI（第三方文件管理器，如 Solid、MT 等）
        try {
            val uri = FileProvider.getUriForFile(activity, "${activity.packageName}.fileprovider", dir)
            val intent = Intent(Intent.ACTION_VIEW).apply {
                setDataAndType(uri, DocumentsContract.Document.MIME_TYPE_DIR)
                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            }
            if (canResolve(intent)) {
                activity.startActivity(intent)
                return
            }
        } catch (_: Exception) {
            // 不在 FileProvider 暴露范围，跳过
        }
        // 3) 回退：打开外部存储根目录，用户自行进入 .lc-sherpa-onnx
        val rootIntent = Intent(Intent.ACTION_VIEW).apply {
            setDataAndType(
                DocumentsContract.buildRootUri(
                    "com.android.externalstorage.documents",
                    "primary"
                ),
                DocumentsContract.Document.MIME_TYPE_DIR
            )
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        }
        if (canResolve(rootIntent)) {
            activity.startActivity(rootIntent)
            return
        }
        // 4) 兜底：复制路径 + 提示
        val clipboard = activity.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        clipboard.setPrimaryClip(ClipData.newPlainText("Sherpa-ONNX 模型目录", dir.absolutePath))
        Toast.makeText(activity, "已复制模型目录: ${dir.absolutePath}", Toast.LENGTH_LONG).show()
    }

    private fun canResolve(intent: Intent): Boolean {
        return activity.packageManager.resolveActivity(intent, PackageManager.MATCH_DEFAULT_ONLY) != null
    }

    private fun hasStoragePermission(): Boolean {
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            Environment.isExternalStorageManager()
        } else {
            val writeGranted =
                activity.checkSelfPermission(Manifest.permission.WRITE_EXTERNAL_STORAGE) ==
                    PackageManager.PERMISSION_GRANTED
            val readGranted =
                activity.checkSelfPermission(Manifest.permission.READ_EXTERNAL_STORAGE) ==
                    PackageManager.PERMISSION_GRANTED
            writeGranted && readGranted
        }
    }

    /** 打开系统「所有文件访问权限」设置页（Android 11+）。 */
    private fun openAllFilesAccessSettings() {
        try {
            val intent = Intent(Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION).apply {
                data = Uri.fromParts("package", activity.packageName, null)
            }
            activity.startActivity(intent)
        } catch (e: Exception) {
            // 部分 ROM 不支持定向跳转，回退到应用详情页（可在其中授予所有文件访问权限）
            val fallback = Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS).apply {
                data = Uri.fromParts("package", activity.packageName, null)
            }
            activity.startActivity(fallback)
        }
    }
}