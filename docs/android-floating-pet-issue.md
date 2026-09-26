# [Feature] 手机端桌宠：Android 系统级悬浮窗（复用主 WebView）

> 本文件是 issue 正文草稿。
> `===== 以下为发布内容（按 .github/ISSUE_TEMPLATE/feature_request.md 填写）=====`
> 之后是**不随 issue 发布**的内部追踪附录。
>
> **已发布**：https://github.com/SlimeBoyOwO/LingChat/issues/842
> （上游没有 `enhancement` 标签，因此未带标签；标题前缀保留。）

===== 以下为发布内容 =====

## 功能描述

让桌宠在 **Android 手机上**可用：一个浮在桌面（其他 App 之上）的小窗，
默认只显示角色头像，点一下展开出输入框，可以拖动、吸附屏幕边缘。

形态与桌面端一致——**同一个 WebView 换形态**，不是另起一个窗口。

## 解决的问题

桌面端桌宠已经能用了，手机上完全没有入口。而手机恰恰是最需要「角色一直陪在
屏幕上」的场景：用户在看视频、刷网页时，角色能浮在上面继续说话。

Android 上「浮在别的 App 之上」只有 `SYSTEM_ALERT_WINDOW` 一条正当路径，
但**把 Tauri 的 WebView 搬进悬浮窗**这件事本身没有先例可抄，所以先做了原型验证。

## 建议的解决方案

### 核心思路：搬运主 WebView，而不是新建

桌面端桌宠的实现是「不是新窗口」——把 `label="main"` 那个窗口改属性
（去边框、缩尺寸、置顶），于是同一个 WebView 从聊天界面变成桌宠，IPC、store、
路由状态全部原样保留。

Android 复刻同一思路：

- wry 用 `activity.setContentView(webView)` 把 Tauri 的 WebView 设成 Activity 的
  根内容视图（`wry-0.55.1/src/android/main_pipe.rs:312`）
- Tauri 的 IPC 是 `webView.addJavascriptInterface(ipc, "ipc")`，
  **绑定在 WebView 对象上，不绑定在窗口上**
- 因此把同一个 View 从 Activity 视图树移到 `WindowManager`，JS 上下文不重载、
  `invoke()` 照常可用、store 数据完整
- 搬走后 Activity 会空掉，先 `setContentView(占位引导页)` 兜底

早期版本在悬浮窗里 `WebView(activity)` 新建实例，结果是个**没有 IPC 的空壳**：
读不到角色数据、发不出消息。这是整个方案的分水岭。

### 尺寸：固定逻辑画布 + 整体等比缩放

**窗口**按屏幕比例算（收起 1/6 屏宽、展开 0.6 屏宽），**页面**不跟着做响应式
布局，而是始终按桌面端那套 240dp 宽的布局排版，再
`transform: scale(窗口宽度 / 240)` 铺满窗口。

让布局跟着窗口走会同时坏三件事：

1. 展开态窗口 2.75 倍宽、头像却锚在收起态宽度 → 窗口里一大片透明区，
   而 Android 悬浮窗**没有逐像素穿透**，那片空白会吃掉下层 App 的触摸
2. 输入框、按钮、字号不跟着缩 → 小窗里挤成一团、点不到
3. 两种形态走不同布局分支，只有一套被真机验证过

代价：文字绝对大小与窗口宽度成正比，所以展开态取 0.6 屏宽而不是 2/5
（2/5 时缩放系数只有 0.6，15px 的字缩到 9px 看不清）。

### 交互

| 手势               | 行为                       |
| ------------------ | -------------------------- |
| 拖动               | 移动窗口，松手吸附左右边缘 |
| 单击头像           | 展开 / 收起来回切换        |
| 展开态左上返回按钮 | 收回悬浮窗并跳回聊天页     |

**没有双击收回**：它和「单击头像展开/收起」物理上无法共存——同一位置的两次
点按，既可能是「展开 → 收起」，也可能是「收回」。

### 不构成架构重构

这一点提前说明：本方案**没有改动任何既有架构**。

- 桌面端桌宠路径（`api::pet::set_pet_mode`）完全不动
- 改动形态是**纯增量**：相对 `upstream/dev` 是 28 文件 +4005/−20，
  其中 19 个是全新文件（独立 crate 的插件 + 一层前端封装 + 文档），
  9 个是既有文件，删除行数总共 20
- 既有文件的改动集中在 4 个前端文件（`PetMode.vue` / `MainChat.vue` /
  `GameRolesStage.vue` / `constants.ts`），其余 5 个是 1–12 行的接线
  （`Cargo.toml` / `Cargo.lock` / `capabilities/default.json` /
  `builder.rs` / `.gitignore`）

## 附加信息

已完成的原型在 fork 分支 `zhangzm0/LingChat#feat/android-floating-pet`
（基线 `9e45571a`，即 `upstream/dev` 上的提交），已过三轮真机验证。

**希望先在这里确认的是：**

1. 这个功能是否符合项目规划、值不值得进主仓库？
2. 如果值得，接受「新增一个独立插件 crate」这种形态吗？
   还是更希望合并进既有的 `api::pet`？
3. 权限方面的顾虑：`SYSTEM_ALERT_WINDOW` + `specialUse` 类型前台服务，
   Google Play 审核较严。这是产品决策，需要维护者定调。

按 `CONTRIBUTING.md` 的「先讨论，后开发」，**在达成共识前我不会提 PR**。

===== 以上为发布内容 =====

---

## 附录：内部追踪（不随 issue 发布）

### 相对 upstream/dev 的规模

```
28 files changed, 4005 insertions(+), 20 deletions(-)
├─ 19 个全新文件  +2255 行（插件 17 个）+ 前端封装 + 本文档
│  ├─ src-tauri/plugins/tauri-plugin-floating-pet/**  （17 个）
│  │  Kotlin 1376（FloatingPetPlugin 1255 + PetForegroundService 121）
│  │  Rust    564（lib 262 + mobile 134 + models 100 + desktop 68）
│  │  gradle / manifest / permissions / build.rs / guest-js
│  ├─ src/api/services/floating-pet.ts
│  └─ docs/android-floating-pet-issue.md
└─  9 个修改文件
   src/components/views/PetMode.vue          +405 −11
   src/components/views/MainChat.vue         +124  −5
   src/components/pet/GameRolesStage.vue      +47  −4
   src/components/pet/constants.ts            +33  −0
   src-tauri/Cargo.lock                       +12  −0
   .gitignore                                  +6  −0
   src-tauri/Cargo.toml                        +2  −0
   src-tauri/capabilities/default.json         +1  −0
   src-tauri/src/app/builder.rs                +1  −0
```

> PR 目标是 `upstream/dev`，**不是** `main`。对 `main` 开 PR 会连带 `dev` 相对
> `main` 的 423 文件 / +46310 行，看起来像雷霆大 diff，实际与本工作无关。

### 几何：为什么必须由原生给，且必须能主动查

缩放系数**不能**由页面从 `window.innerWidth` 推：原生 `updateViewLayout` 之后
WebView 的视口要过一会儿才跟上，这中间读到的宽度是滞后的，算出的系数偏小
→ 内容只占窗口一角、展开后一大片空白。更糟的是页面还会把这个滞后值当**宽度**
回传给 `set_size`，把刚展开的窗口又缩回收起态。

约定：

- 宽度由**原生独占**（只有它知道真实屏幕宽度）；`set_size` 的 `width <= 0`
  语义是「只改高度」
- 高度由**页面**按实际内容算好后回报（气泡是流式输出的，写死必然算错）
- `scale = view.width / density / 240`，取 View 的**实际布局尺寸**而不是
  `params.width`（系统可能因 insets / 多窗口调整过窗口）

传输上有两条路，**以第二条为准**：

| 方向        | 手段                               | 可靠性                       |
| ----------- | ---------------------------------- | ---------------------------- |
| 原生 → 页面 | `evaluateJavascript` 派发 DOM 事件 | 搬运/收回前后**不可靠**      |
| 页面 → 原生 | Tauri IPC 轮询 `status` 命令       | **稳**（点击、发消息都走它） |

页面每 500ms 查一次 `status`（`detached` / `scale` / `width` / `height`），
「我已经被搬回 Activity 了」也由它判断。`pet-metrics` 事件保留为快路径，
但正确性不再依赖它。

### WebView 保活：Tauri 插件生命周期是死代码

App 退到后台后桌宠会静止不动。根因链条（可复核）：

- `WryActivity.onPause()` **无条件**调用 `mWebView.onPause()`
  （`wry-0.55.1/src/android/kotlin/WryActivity.kt:130-135`），
  它并不知道这个 WebView 已被搬进 WindowManager
- 此后没有任何代码路径会再调 `onResume()`，除非 Activity 自己回到前台
- 想用插件的 `onPause/onResume` 补救走不通：Tauri 2.11.1 里
  `PluginManager.onPause/onResume/onStop` 唯一的上游是 `TauriLifecycleObserver`，
  而它只被定义、**从未被 `addObserver()` 注册**
  （`tauri-2.11.1/mobile/android-codegen/TauriActivity.kt`）——
  即 `Plugin.onPause/onResume/onStop` 目前是**死代码**

现在：`PetForegroundService` 保**进程**，主线程 Handler 每 500ms 调
`onResume()` + `resumeTimers()` 保 **WebView**，收回后仍多撑 20 秒等视口重算完；
用户切回 App 时由 `Application.ActivityLifecycleCallbacks` 补发漏掉的收回事件。
进程存活与 WebView 存活是**两个独立问题**，缺一不可。

### 已知问题 / 待办

- [ ] **收回后仍停在悬浮窗布局**（真机反馈：即使在前台收回也会出现）。
      本轮已改为页面主动查询 `status` + 乐观切换本地状态，待真机复验
- [ ] **展开后大片透明区**（真机反馈：空白区域在宠物之外但仍可触摸）。
      本轮已把缩放系数改为原生下发 + 轮询，待真机复验
- [ ] 收起态不显示气泡：缩放系数只有 0.25，字会小到看不清
- [ ] `isVisible()` 只读 Rust 侧内存状态，App 重启后丢失
- [ ] 展开态点头像不再推进对话（手势被收起占用），只能靠发送消息 / 自动模式
- [ ] 桌面端那排「悬停才浮现」的按钮在悬浮窗里一律隐藏
- [ ] 未做权限引导 UI：授权页返回后需用户再点一次

### PR 计划（达成共识后）

19 个提交整理成 3–4 个语义化提交：

1. `feat(android): 桌宠系统级悬浮窗插件`（插件本体 + 能力/权限，纯新增）
2. `feat(android): 前端接入悬浮桌宠`（`floating-pet.ts` + 4 个前端文件）
3. `fix(android): 真机问题修复`（保活、几何、手势）
4. `docs(android): 补充架构说明与测试步骤`

PR 描述按 `.github/PULL_REQUEST_TEMPLATE/feature.md` 填写。

### 风险

- **上架政策**：Google Play 对 `SYSTEM_ALERT_WINDOW` 审核严格，需陈述必要性；
  `specialUse` 前台服务还要声明 `PROPERTY_SPECIAL_USE_FGS_SUBTYPE`
- **厂商限制**：小米/华为/OPPO 等除「显示在其他应用上层」，还需在
  「后台弹出界面」「自启动」里放行，否则悬浮窗被静默拦截
- **iOS 不支持**：系统不允许第三方 App 创建跨 App 覆盖窗口，这条路径到 iOS 就断

### 验证方式

Android 改动**必须走 CI**：本地 `cargo check` 只覆盖桌面端，
`#[cfg(target_os = "android")]` 路径根本不编译；而 Android 交叉编译受
`ring` / `aws-lc-sys` 的 C/汇编工具链限制。

```bash
gh workflow run dev-build-android.yml --ref feat/android-floating-pet
gh run watch <run-id> --exit-status
```

CI 完整编译 Rust（android target）+ Kotlin 并产出签名 APK。已借此发现并修复
两个本地不可见的错误：`run_mobile_plugin` 的宿主类型、`View` / `WebView`
的方法归属。真机测试步骤见 `docs/android-floating-pet.md` 第 7 节。
