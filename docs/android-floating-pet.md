# 手机端桌宠（Android 系统悬浮窗）

> 状态：**P1 / P2 已完成**（搬运主 WebView + 手机端手势交互）
> 分支：`feat/android-floating-pet`（基于 `upstream/dev`）

## 一、背景与目标

桌面端的桌宠是一个「透明 + 置顶 + 无边框 + 可点击穿透」的原生小窗口，由
`src-tauri/src/api/pet.rs` 实现。手机端希望得到**类似的小窗体验**——一个能浮在
其他 App 之上、可拖拽的小角色，而不是全屏页面。

Android 没有等价的窗口概念，需要走 `SYSTEM_ALERT_WINDOW` 权限 +
`WindowManager.addView()` 创建系统级覆盖窗口。Tauri 不提供该能力，因此实现为
一个本地插件。

**核心设计取向**：桌面端桌宠是「同一个窗口换一身属性」，手机端也应如此——
把**主 WebView 本身**搬进悬浮窗，而不是新建一个。详见第四节。

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

## 三、插件结构

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
│           └── FloatingPetPlugin.kt    # WebView 搬运 + 悬浮窗核心
├── permissions/default.toml
└── guest-js/index.ts
```

### 3.1 平台能力矩阵

| 平台    | 支持 | 实现                                                   |
| ------- | ---- | ------------------------------------------------------ |
| Android | ✅   | `TYPE_APPLICATION_OVERLAY` 悬浮窗 + **搬运主 WebView** |
| 桌面端  | ❌   | 由 `api::pet` 的原生窗口实现（降级为 `NotSupported`）  |
| iOS     | ❌   | 系统不允许跨 App 覆盖窗口                              |

### 3.2 命令清单

| 命令                    | 说明                                     |
| ----------------------- | ---------------------------------------- |
| `is_supported`          | 平台是否支持                             |
| `check_permission`      | 是否已获悬浮窗权限                       |
| `request_permission`    | 跳转系统授权页                           |
| `show`                  | **把主 WebView 搬进悬浮窗**（幂等）      |
| `hide`                  | **把 WebView 搬回 Activity**，恢复主界面 |
| `set_expanded`          | 展开/收起（改窗口尺寸 + 通知页面切布局） |
| `move_pet`              | 移动坐标                                 |
| `set_size`              | 调整尺寸                                 |
| `set_touchable`         | 切换点击穿透                             |
| `is_visible` / `status` | 状态查询                                 |

### 3.3 前端接入

`src/api/services/floating-pet.ts` 提供封装。进入流程是
**先切页 → 再搬移**（`MainChat.vue` 的 `goToPetMode`）：

```ts
await router.push("/pet"); // ① 先切页
await showFloatingPet({ scale: settingsStore.pet?.scale }); // ② 再搬移
```

尺寸**不由前端指定**：手机端按屏幕宽度比例在 Kotlin 侧计算
（收起态 1/6 屏宽、展开态 2/5 屏宽），只有原生知道真实屏幕宽度。
比例常量在两边各有一份，必须同步（`src/components/pet/constants.ts`
的 `MOBILE_*_RATIO` 与 Kotlin 的 `COLLAPSED_*` / `EXPANDED_*`）。

## 四、核心架构：搬运主 WebView

### 4.1 为什么不新建 WebView

桌面端的桌宠**不是新窗口**：它把 `label="main"` 那个窗口改属性
（`set_decorations(false)` + `set_size` + `set_always_on_top`），于是**同一个
WebView** 从「聊天界面」变成「桌宠」，IPC、store、路由状态全部原样保留。

早期版本在悬浮窗里 `WebView(activity)` 新建了实例，结果是一个**没有 IPC 的空壳**：
读不到角色数据、发不出消息，只能渲染静态页面。这是被否决的方案。

### 4.2 搬运为什么可行

三个事实支撑这个方案（均已核对源码）：

1. wry 用 `activity.setContentView(webView)` 把 Tauri 的 WebView 设为 Activity 的
   **根内容视图**（见 wry `android/main_pipe.rs`）。
2. Tauri 的 IPC 是 `webView.addJavascriptInterface(ipc, "ipc")` —— **绑定在 WebView
   对象上，不绑定在窗口上**。
3. 因此把同一个 View 从 Activity 视图树移到 `WindowManager`，JS 上下文不重载、
   `invoke()` 照常可用、store 数据完整。

### 4.3 占位页解决「Activity 变空」

WebView 是 Activity 的唯一内容视图，搬走后 Activity 就空了（白屏）。
因此在 `addView` 之前先 `setContentView(占位页)`，用户切回 App 时看到的是
一张引导图而不是空白。

流程：

```
进入桌宠：
  ① 前端 router.push('/pet')          ← 先切页，此时还在屏幕内
  ② 原生 removeView(webView)          ← 从 Activity 视图树摘下
  ③ 原生 setContentView(占位页)        ← 防白屏
  ④ 原生 addView(webView, 悬浮窗参数)  ← 桌宠出现在桌面上

收回：
  ⑤ 原生 removeView(webView)
  ⑥ 原生 setContentView(webView)      ← 主界面原样回来
  ⑦ 前端 router.push('/chat')
```

**顺序不能反**：先搬移再切页的话，用户会看到主界面闪一下才变成桌宠。

### 4.4 两个必须知道的平台限制

**点击穿透是窗口级开关，无法按区域精细控制**

桌面端（Windows）用 `GetCursorPos` 轮询 + `set_ignore_cursor_events`，可以按
**像素区域**判定是否穿透。Android 的 `FLAG_NOT_TOUCHABLE` 只能作用于**整个窗口**。

**应对**：窗口尺寸按屏幕比例收窄（收起态仅 1/6 屏宽），不做大块透明留白。

**WebView 的生命周期归 Activity 管**

`WryActivity.mWebView` 是 `lateinit`，`onPause/onResume/onDestroy` 都会直接访问它。
搬运期间这些回调可能干扰悬浮窗里的 WebView。`onDestroy` 时插件会主动移除悬浮窗，
避免留下「僵尸窗口」（宿主 Activity 已销毁，宠物还在屏幕上且关不掉）。

## 五、手机端的交互设计

### 5.1 尺寸按屏幕比例，不按固定 dp

桌面端固定 240×480dp。手机屏宽普遍 360–430dp，直接搬过来会让桌宠占到
**58%–67% 屏宽**（这正是早期「占了手机一半屏幕」的原因）。

改为按屏幕宽度比例计算（Kotlin 侧 `screenWidthDp()`）：

| 状态 | 宽度     | 高度比 | 显示内容             |
| ---- | -------- | ------ | -------------------- |
| 收起 | 1/6 屏宽 | ×1.15  | 仅头像 + 操作提示    |
| 展开 | 2/5 屏宽 | ×2.0   | 头像 + 气泡 + 输入框 |

### 5.2 手势：拖动 / 单击 / 双击

手机没有鼠标，桌面端那套 `mouseenter/mouseleave` 完全不适用：

| 手势       | 行为                                     |
| ---------- | ---------------------------------------- |
| 拖动       | 移动窗口，松手后吸附到最近的左右边缘     |
| 单击头像   | 展开输入框（收起态）／推进对话（展开态） |
| 双击       | 收回悬浮窗，切回 App                     |
| 展开后点 ✕ | 收回悬浮窗                               |

**为什么收回是双击而不是单击**：单击已经被「点头像展开」占用，两者会直接冲突
——用户想展开，结果被弹回 App。双击是移动端常见的「返回/退出」语义，因此用它。

拖动与点击的区分靠位移阈值（`TAP_SLOP_DP = 8dp`）：没有它，每次拖完都会误触发点击。

## 六、后续阶段

| 阶段      | 目标             | 关键工作                                          |
| --------- | ---------------- | ------------------------------------------------- |
| **P0** ✅ | 技术可行性验证   | 插件骨架、CI 编译通过                             |
| **P1** ✅ | 搬运主 WebView   | 占位页、view 搬运、TCP 保留（IPC/store 完整可用） |
| **P2** ✅ | 手机端交互       | 屏幕比例尺寸、拖动/单击/双击、展开收起            |
| **P3**    | 气泡悬浮到窗口外 | 利用 `FLAG_LAYOUT_NO_LIMITS` 让气泡画出窗口边界   |
| **P4**    | 打磨发布         | 权限引导 UI、前台服务保活、厂商白名单             |

## 六、发布前必须处理的风险

| 风险         | 说明                                                                        |
| ------------ | --------------------------------------------------------------------------- |
| **上架政策** | Google Play 对 `SYSTEM_ALERT_WINDOW` 审核严格，需陈述必要性；国内商店较宽松 |
| **厂商限制** | 小米/华为/OPPO 等需额外加入「后台弹出界面」白名单，否则悬浮窗被拦截         |
| **后台存活** | 需 `FOREGROUND_SERVICE` + 常驻通知，否则切后台后悬浮窗可能被回收            |
| **权限引导** | 该权限无法运行时弹窗申请，只能跳设置页；用户返回后需重新检测                |

> 注：本项目为 **AGPL-3.0**，代码本就要求开源，无闭源商业化顾虑。

## 七、怎么测试

### 7.1 拿安装包

CI 每次构建都产出 APK artifact（不上架、不建 Release）：

```bash
gh workflow run dev-build-android.yml --repo <你的fork> --ref feat/android-floating-pet
gh run download <run-id> --repo <你的fork> -n lingchat-dev-android
```

也可以本地出包（需要能跑 Android 工具链的机器）：

```bash
pnpm android:devbuild    # debug APK，装起来最快
```

### 7.2 真机验证步骤

1. 安装 APK，启动 App，进入聊天主界面
2. 点右上角**「桌宠」**按钮
3. **首次**会提示需要悬浮窗权限 → 跳系统「显示在其他应用上层」设置页
   → 打开 LingChat 开关 → 返回 App
4. **再点一次「桌宠」** → 短暂切到 `/pet` 页后，WebView 被搬进悬浮窗，
   桌面上出现**仅头像**的小窗（约 1/6 屏宽）
5. 此时切回 App：应看到**占位引导页**（而不是白屏），说明 WebView 已搬走
6. **单击头像** → 窗口变大到约 2/5 屏宽，出现输入框与右上角 ✕
7. **在输入框里发一条消息** → 应能正常发送并收到回复
   （这是搬运方案的核心验证点：IPC 与 store 都还在）
8. **拖动头像** → 窗口跟随移动，松手后吸附到屏幕边缘
9. **双击头像** → 收回悬浮窗，回到 App 主界面

### 7.3 当前能验证到哪一步

| 能力                      | 状态                                          |
| ------------------------- | --------------------------------------------- |
| 搬运主 WebView 进悬浮窗   | ✅                                            |
| 浮在其他 App 之上         | ✅                                            |
| Activity 占位页（防白屏） | ✅                                            |
| **IPC / store 完整可用**  | ✅ 这是搬运方案相对「新建 WebView」的关键收益 |
| 角色/台词显示             | ✅ 数据来自原有 store，无需镜像               |
| 在悬浮窗里发消息          | ✅ 复用原有 `ChatInput` 与 Tauri 命令         |
| 拖动移动 + 边缘吸附       | ✅                                            |
| 单击展开 / 双击收回       | ✅                                            |
| 气泡画出窗口外            | ❌ 未接（P3）                                 |

### 7.4 权限被拒 / 找不到开关

- **国产 ROM**：小米/华为/OPPO 等除「显示在其他应用上层」，还需在
  「后台弹出界面」「自启动」里放行，否则悬浮窗被静默拦截
- **没弹设置页**：部分系统该权限默认关闭且无入口，属系统限制
- **排查**：看 logcat 中 `FloatingPet` 标签的输出

### 7.5 已知风险点（真机重点观察）

| 现象                     | 可能原因                                               |
| ------------------------ | ------------------------------------------------------ |
| 切回 App 白屏            | 占位页没生效，`setContentView` 顺序有问题              |
| 收回后主界面黑屏         | WebView 搬回失败，需看 `restoreWebViewToActivity` 日志 |
| 悬浮窗里输入框弹不出键盘 | 窗口 `FLAG_NOT_FOCUSABLE` 与输入法冲突（P3 处理）      |
| 拖动后误触发收回         | 位移阈值偏小，需调大 `TAP_SLOP_DP`                     |
| 按住 Home 后悬浮窗消失   | 系统回收，需 `FOREGROUND_SERVICE` 保活（P4）           |

## 八、工程验证方式

Rust 侧本地 `cargo check` 只能验证桌面端，**移动端代码路径
（`#[cfg(target_os = "android")]`）本地测不出**——桌面端不编译 `mobile.rs`，
而 Android 交叉编译受 `ring` / `aws-lc-sys` 的 C/汇编工具链限制。

因此 Android 改动**必须走 CI**：

```bash
gh workflow run dev-build-android.yml --ref <branch>
gh run watch <run-id> --exit-status
```

CI 完整编译 Rust（android target）+ Kotlin 并产出 APK。P0 阶段已借此发现并
修复两个本地不可见的错误（`run_mobile_plugin` 的宿主类型、参数缺 `Serialize`）。
