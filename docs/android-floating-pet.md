# 手机端桌宠（Android 系统悬浮窗）调研与 P0 验证

> 状态：**P0 已完成**（插件骨架 + CI 编译验证通过）
> 分支：`feat/android-floating-pet`（基于 `upstream/dev`）

## 一、背景与目标

桌面端的桌宠是一个「透明 + 置顶 + 无边框 + 可点击穿透」的原生小窗口，由
`src-tauri/src/api/pet.rs` 实现。手机端希望得到**类似的小窗体验**——一个能浮在
其他 App 之上、可拖拽的小角色，而不是全屏页面。

Android 没有等价的窗口概念，需要走 `SYSTEM_ALERT_WINDOW` 权限 +
`WindowManager.addView()` 创建系统级覆盖窗口。Tauri 不提供该能力，因此实现为
一个本地插件。

## 二、三条技术路线的取舍

|                 | Activity Embedding（Tauri 原生） | **系统悬浮窗（本方案）** | App 内悬浮       |
| --------------- | -------------------------------- | ------------------------ | ---------------- |
| 是否真·小窗     | ❌ 手机上整屏替换                | ✅ 真悬浮窗              | ⚠️ 仅 App 内浮动 |
| 浮在其他 App 上 | ❌                               | ✅                       | ❌               |
| 需原生代码      | ❌                               | ✅ Kotlin                | ❌               |
| 需特殊权限      | ❌                               | ✅ 悬浮窗权限            | ❌               |
| 实现难度        | 低                               | 高                       | 低               |

Tauri v2 的多窗口在 Android 上基于 Activity Embedding，**手机上系统不会并排布局**，
第二个 Activity 会占满全屏并压入返回栈——达不到桌宠的效果，故排除。

## 三、P0 已落地内容

### 3.1 插件结构

```
src-tauri/plugins/tauri-plugin-floating-pet/
├── Cargo.toml
├── build.rs                    # 命令清单 + android_path("android")
├── src/
│   ├── lib.rs                  # 插件入口、状态、命令注册、Android 插件注册
│   ├── desktop.rs              # 桌面端：统一 NotSupported
│   ├── mobile.rs               # Android：转发到 Kotlin
│   └── models.rs               # 参数与错误模型
├── android/
│   ├── build.gradle.kts
│   └── src/main/
│       ├── AndroidManifest.xml         # SYSTEM_ALERT_WINDOW 声明
│       └── java/com/noiq/floatingpet/
│           └── FloatingPetPlugin.kt    # WindowManager 悬浮窗核心
├── permissions/default.toml
└── guest-js/index.ts
```

### 3.2 平台能力矩阵

| 平台    | 支持 | 实现                                                  |
| ------- | ---- | ----------------------------------------------------- |
| Android | ✅   | `TYPE_APPLICATION_OVERLAY` 悬浮窗 + 内嵌透明 WebView  |
| 桌面端  | ❌   | 由 `api::pet` 的原生窗口实现（降级为 `NotSupported`） |
| iOS     | ❌   | 系统不允许跨 App 覆盖窗口                             |

### 3.3 命令清单

| 命令                    | 说明                               |
| ----------------------- | ---------------------------------- |
| `is_supported`          | 平台是否支持                       |
| `check_permission`      | 是否已获悬浮窗权限                 |
| `request_permission`    | 跳转系统授权页                     |
| `show`                  | 显示悬浮窗（幂等，重复调用先隐藏） |
| `hide`                  | 隐藏                               |
| `move_pet`              | 移动坐标                           |
| `set_size`              | 调整尺寸                           |
| `set_touchable`         | 切换点击穿透                       |
| `is_visible` / `status` | 状态查询                           |

### 3.4 前端接入

`src/api/services/floating-pet.ts` 提供封装，`enterFloatingPet()` 一次完成
「探测能力 → 检查授权 → 引导授权 → 显示」全流程。

尺寸计算复用 `src/components/pet/constants.ts` 的基准值，与桌面端窗口尺寸
（`api/pet.rs`）保持同源，保证两端视觉一致。

## 四、两个必须知道的平台限制

### 4.1 点击穿透是窗口级开关，无法按区域精细控制

桌面端（Windows）用 `GetCursorPos` 轮询 + `set_ignore_cursor_events`，可以按
**像素区域**判定是否穿透。Android 的 `FLAG_NOT_TOUCHABLE` 只能作用于**整个窗口**。

**应对**：悬浮窗尺寸按宠物本身收窄（见 `calcFloatingPetSize`），不做大块透明留白，
避免挡住下层 App 的触摸。

### 4.2 悬浮窗内是独立 WebView，不在 Tauri IPC 上下文

悬浮窗里的 WebView 加载 `/pet` 路由，它可以正常渲染角色、播放语音，但
**不能直接 `invoke()` Tauri 命令**。

**当前状态**：`PetMode.vue` 的 `onMounted` 会调用 `getCurrentWindow()` 等
Tauri 窗口 API，在悬浮窗环境下会失败。

**P1 必须处理**：给 `PetMode.vue` 增加悬浮窗模式分支，跳过窗口相关调用
（`set_pet_mode`、`update_solid_regions`、`WebviewWindow` 等）。

**通信方案**（P2）：主界面 ↔ 悬浮窗走 `Plugin.trigger` 事件单向推送。

## 五、后续阶段

| 阶段      | 目标                 | 关键工作                                        |
| --------- | -------------------- | ----------------------------------------------- |
| **P0** ✅ | 技术可行性验证       | 插件骨架、CI 编译通过                           |
| **P1**    | 悬浮窗能正确显示桌宠 | `PetMode.vue` 加悬浮窗分支，跳过窗口 API        |
| **P2**    | 交互可用             | 拖拽移动、触摸/穿透切换、事件通道               |
| **P3**    | 与主 App 联动        | 状态同步（单向推送）、语音同步                  |
| **P4**    | 打磨发布             | 权限引导 UI、边缘吸附、前台服务保活、厂商白名单 |

## 六、发布前必须处理的风险

| 风险         | 说明                                                                        |
| ------------ | --------------------------------------------------------------------------- |
| **上架政策** | Google Play 对 `SYSTEM_ALERT_WINDOW` 审核严格，需陈述必要性；国内商店较宽松 |
| **厂商限制** | 小米/华为/OPPO 等需额外加入「后台弹出界面」白名单，否则悬浮窗被拦截         |
| **后台存活** | 需 `FOREGROUND_SERVICE` + 常驻通知，否则切后台后悬浮窗可能被回收            |
| **权限引导** | 该权限无法运行时弹窗申请，只能跳设置页；用户返回后需重新检测                |

> 注：本项目为 **AGPL-3.0**，代码本就要求开源，无闭源商业化顾虑。

## 七、验证方式

Rust 侧本地 `cargo check` 可验证桌面端编译，但**移动端代码路径
（`#[cfg(target_os = "android")]`）本地无法验证**——本项目桌面端不编译
`mobile.rs`，而 Android 交叉编译在本机环境受 `ring` / `aws-lc-sys` 的
C/汇编工具链限制。

因此 Android 相关改动**必须走 CI 验证**：

```bash
gh workflow run dev-build-android.yml --ref <branch>
gh run watch <run-id> --exit-status
```

CI 会完整编译 Rust（android target）+ Kotlin，并产出 APK artifact。
