# [追踪] 手机端桌宠：Android 系统级悬浮窗（搬运主 WebView）

> 本文是 issue 正文草稿，用于跟踪手机端桌宠从调研到可 PR 的全过程。
> 分支：`feat/android-floating-pet`（fork `zhangzm0/LingChat`）
> 基线：`9e45571a`（该提交在 `upstream/dev` 上，**不在** `upstream/main` 上）

## 背景

桌面端桌宠的实现是「**不是新窗口**」：把 `label="main"` 那个窗口改属性
（去边框、缩尺寸、置顶），于是同一个 WebView 从「聊天界面」变成「桌宠」，
IPC、store、路由状态全部原样保留。

手机端想要同一个东西，但 Android 上「浮在别的 App 之上」只有一条正当路径：
`SYSTEM_ALERT_WINDOW` 系统悬浮窗。于是问题变成：

**能不能把主 WebView 搬进悬浮窗，而不是在悬浮窗里新建一个？**

## 目标形态

- 默认是**只有头像**的小窗（约 1/6 屏宽），浮在桌面上
- 单击头像 → 动态展开出输入框（约 0.6 屏宽）
- 展开态左上角有返回按钮 → 收回悬浮窗并回到 App
- 可拖动，松手吸附屏幕左右边缘

## 当前状态

功能已跑通并经过三轮真机验证。相对 `upstream/dev` 的规模：

```
27 files changed, 3530 insertions(+), 20 deletions(-)
├─ 18 个全新文件  +2255 行（插件本体 17 个 + 前端封装）
└─  9 个修改文件  其中真正需要 review 的是 4 个前端文件
```

删除行数总共只有 20 —— 基本是纯增量，不改既有逻辑。

## 架构决策

### 1. 搬运主 WebView，而不是新建

wry 用 `activity.setContentView(webView)` 把 Tauri 的 WebView 设成 Activity 的
根内容视图（`wry-0.55.1/src/android/main_pipe.rs:312`），而 Tauri 的 IPC 是
`webView.addJavascriptInterface(ipc, "ipc")` —— **绑定在 WebView 对象上，不绑定在窗口上**。

因此把同一个 View 从 Activity 视图树移到 `WindowManager`：
JS 上下文不重载、`invoke()` 照常可用、store 数据完整。

早期版本在悬浮窗里 `WebView(activity)` 新建实例，结果是个**没有 IPC 的空壳**：
读不到角色数据、发不出消息。这是整个方案的分水岭。

搬走后 Activity 会空掉，因此先 `setContentView(占位引导页)`。

### 2. 固定逻辑画布 + 整体等比缩放

**窗口**按屏幕比例算，**页面**却**不**跟着做响应式布局——而是始终按桌面端
那套 240dp 宽的布局排版，再 `transform: scale(窗口宽度 / 240)` 铺满窗口。

试过让布局跟着窗口走，三件事同时坏掉：

1. 展开态窗口 2.75 倍宽、头像却锚在收起态宽度 → 窗口里一大片透明区，
   而 Android 悬浮窗**没有逐像素穿透**，那片空白会**吃掉下层 App 的触摸**
2. 输入框、按钮、字号不跟着缩 → 小窗里挤成一团、点不到
3. 两种形态走不同布局分支，只有一套被真机验证过

代价要说清楚：**文字绝对大小与窗口宽度成正比**。所以展开态宽度取 0.6 屏宽
而不是 2/5 —— 2/5 屏宽（144dp）时缩放系数只有 0.6，15px 的字缩到 9px 看不清。

> 顺带纠正一个错误认知：`FLAG_LAYOUT_NO_LIMITS` 是让**窗口本身**可以超出
> 屏幕边界，**不是**让 WebView 内容画出自己的 bounds。WebView 内容永远被
> 裁剪在自身范围内，气泡不可能靠它跑到窗口外。

### 3. 几何由原生下发 + 页面主动查询

缩放系数**不能**由页面从 `window.innerWidth` 推：原生 `updateViewLayout` 之后
WebView 的视口要过一会儿才跟上，这中间读到的宽度是滞后的，算出的系数偏小
→ 内容只占窗口一角、展开后一大片空白。

更糟的是页面还会把这个滞后值当**宽度**回传给 `set_size`，把刚展开的窗口
又缩回收起态。

现在的约定：

- 宽度由**原生独占**（只有它知道真实屏幕宽度）；`set_size` 的 `width <= 0`
  语义是「只改高度」
- 高度由**页面**按实际内容算好后回报（气泡是流式输出的，高度写死必然算错）
- 缩放系数 `scale = 窗口实际宽度(dp) / 240`，由原生计算

传输上有两条路，**以第二条为准**：

| 方向        | 手段                               | 可靠性                       |
| ----------- | ---------------------------------- | ---------------------------- |
| 原生 → 页面 | `evaluateJavascript` 派发 DOM 事件 | 搬运/收回前后**不可靠**      |
| 页面 → 原生 | Tauri IPC 轮询 `status` 命令       | **稳**（点击、发消息都走它） |

页面每 500ms 查一次 `status`，拿 `detached` / `scale` / `width` / `height`。
「我已经被搬回 Activity 了」也由它判断，不再赌事件有没有送达。

### 4. WebView 保活：Tauri 插件生命周期是死代码

App 退到后台后桌宠会静止不动。根因链条（可复核）：

- `WryActivity.onPause()` **无条件**调用 `mWebView.onPause()`
  （`wry-0.55.1/src/android/kotlin/WryActivity.kt:130-135`），
  它并不知道这个 WebView 已被搬进 WindowManager
- 此后没有任何代码路径会再调 `onResume()`，除非 Activity 自己回到前台
- 想用插件的 `onPause/onResume` 补救走不通：Tauri 2.11.1 里
  `PluginManager.onPause/onResume/onStop` 唯一的上游是 `TauriLifecycleObserver`，
  而它只被定义、**从未被 `addObserver()` 注册**
  （`tauri-2.11.1/mobile/android-codegen/TauriActivity.kt`）——
  也就是说 `Plugin.onPause/onResume/onStop` 目前是**死代码**

现在的做法：

- `PetForegroundService`（前台服务）保**进程**
- 主线程 Handler 每 500ms 调 `webView.onResume()` + `resumeTimers()` 保 **WebView**
- 收回后仍多撑 20 秒，等 Chromium 把视口重算完再停
- 用户切回 App 时由 `Application.ActivityLifecycleCallbacks` 补发漏掉的收回事件

进程存活与 WebView 存活是**两个独立问题**，缺一不可。

## 交互约定

| 手势               | 行为                       |
| ------------------ | -------------------------- |
| 拖动               | 移动窗口，松手吸附左右边缘 |
| 单击头像           | 展开 / 收起来回切换        |
| 展开态左上返回按钮 | 收回悬浮窗并跳回聊天页     |

**没有双击收回。** 它和「单击头像展开/收起」物理上无法共存：同一位置的
两次点按，既可能是「展开 → 收起」，也可能是「收回」。早期版本的判定还只看
时间不看位置，任意两次 300ms 内的点按都算双击（点完头像紧接着点输入框也会
把桌宠收回去），真机误触严重。

## 已知问题 / 待办

- [ ] **收回后仍停在悬浮窗布局**（真机反馈：即使在前台收回也会出现）。
      本轮已改为页面主动查询 `status` + 乐观切换本地状态，待真机复验
- [ ] **展开后大片透明区**（真机反馈：空白区域在宠物之外但仍可触摸）。
      本轮已把缩放系数改为原生下发 + 轮询，待真机复验
- [ ] 收起态不显示气泡：缩放系数只有 0.25，字会小到看不清
- [ ] `isVisible()` 只读 Rust 侧内存状态，App 重启后丢失
- [ ] 展开态点头像不再推进对话（手势被收起占用），推进只能靠发送消息 / 自动模式
- [ ] 桌面端那排「悬停才浮现」的按钮（设置/自动/返回主页/截图/语音）在悬浮窗里
      一律隐藏：手机没有 hover，且缩放后会被 `overflow-hidden` 裁掉
- [ ] 未做权限引导 UI：授权页返回后需用户再点一次

## PR 计划

目标是 `upstream/dev`（**不是** `main`）。

对 `main` 开 PR 会连带 `dev` 相对 `main` 的 423 文件 / +46310 行，
看起来像雷霆大 diff，实际与本工作无关。

建议把 19 个提交整理成 3–4 个语义化提交，再开 PR：

1. `feat(android): 桌宠系统级悬浮窗插件`（插件本体 + 能力/权限，纯新增）
2. `feat(android): 前端接入悬浮桌宠`（`floating-pet.ts` + `PetMode/MainChat/GameRolesStage/constants`）
3. `fix(android): 真机问题修复`（保活、几何、手势）
4. `docs(android): 补充架构说明与测试步骤`

## 风险

- **上架政策**：Google Play 对 `SYSTEM_ALERT_WINDOW` 审核严格，需陈述必要性；
  `specialUse` 类型的前台服务还要声明 `PROPERTY_SPECIAL_USE_FGS_SUBTYPE`。
  这是产品决策，不是代码问题，但大概率是 PR 讨论的焦点
- **厂商限制**：小米/华为/OPPO 等除「显示在其他应用上层」，还需在
  「后台弹出界面」「自启动」里放行，否则悬浮窗被静默拦截
- **iOS 不支持**：系统不允许第三方 App 创建跨 App 覆盖窗口，这条路径到 iOS 就断了

## 验证方式

Android 改动**必须走 CI**：本地 `cargo check` 只覆盖桌面端，
`#[cfg(target_os = "android")]` 路径根本不编译；而 Android 交叉编译受
`ring` / `aws-lc-sys` 的 C/汇编工具链限制。

```bash
gh workflow run dev-build-android.yml --ref feat/android-floating-pet
gh run watch <run-id> --exit-status
```

CI 完整编译 Rust（android target）+ Kotlin 并产出签名 APK。
P0 阶段已借此发现并修复两个本地不可见的错误：
`run_mobile_plugin` 的宿主类型、以及 `View` / `WebView` 的方法归属。

真机测试步骤见 `docs/android-floating-pet.md` 第 7 节。
