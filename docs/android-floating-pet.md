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
**像素区域**判定是否穿透。Android 的 `FLAG_NOT_TOUCHABLE` 只能作用于**整个窗口**，
而且它在**按下那一刻**就被求值——没有「先看手指落在哪再决定穿不穿透」的余地。

**应对**：窗口尺寸按屏幕比例收窄（收起态仅 1/6 屏宽），不做大块透明留白。

> ⚠️ **残留问题（未解决）**：悬浮窗是矩形，宠物是圆形，所以展开态必然有一圈
> 透明角落吃触摸。这部分**无法**用现有 API 消除——这是「矩形系统窗口 + 圆形宠物」
> 的固有代价，不是 bug。能做的只有把矩形收紧到宠物的可见外接矩形。
> 真机上先看诊断里的两组数字：
>
> - `band=`（气泡带 `offsetHeight`）：不为 0 说明气泡带占了高度却没渲染出内容，
>   会在宠物与输入框**中间**留出一条透明带
> - `role scaleP=`：角色卡的「桌宠缩放」。它 < 1 时宠物只占头像框的一部分，
>   四周全是透明区。桌面端靠点击穿透忽略这些区域，Android 上则会吃掉触摸
>
> 诊断代码在 `PetMode.vue` 的 `DEBUG_FLOATING_OVERLAY` 段落，**合并前必须删除**。

**WebView 的生命周期归 Activity 管**

`WryActivity.mWebView` 是 `lateinit`，`onPause/onResume/onDestroy` 都会直接访问它。
搬运期间这些回调可能干扰悬浮窗里的 WebView。`onDestroy` 时插件会主动移除悬浮窗，
避免留下「僵尸窗口」（宿主 Activity 已销毁，宠物还在屏幕上且关不掉）。

**App 退到后台 → 悬浮窗里的 WebView 被冻住（已踩过的坑）**

wry 0.55.1 的 `WryActivity.onPause()` 会**无条件**调 `mWebView.onPause()`
（`WryActivity.kt:130-135`），它并不知道这个 WebView 已经被搬进 WindowManager。
于是 App 一退后台：Activity 暂停 → 桌宠静止不动、`evaluateJavascript` 也不再执行
（原生派发的 `pet-attached` 等事件会全部丢失），而且此后**没有任何代码路径**
会再调 `onResume()`，除非 Activity 自己回到前台。

> ⚠️ 想靠插件的 `onPause/onResume` 钩子补救是走不通的。Tauri 2.11.1 里
> `PluginManager.onPause/onResume/onStop` 唯一的上游是 `TauriLifecycleObserver`，
> 而它只被定义、**从未被 `addObserver()` 注册**（见
> `tauri-2.11.1/mobile/android-codegen/TauriActivity.kt`）——也就是说
> `Plugin.onPause/onResume/onStop` 目前是**死代码**。`TauriActivity` 只覆写了
> `onCreate / onNewIntent / onRestart / onDestroy / onConfigurationChanged`。

**应对**：`FloatingPetPlugin.startKeepAlive()` —— 不依赖任何生命周期回调，
用主线程 Handler 每 500ms 轮询一次。

> ⚠️ 轮询里**不能**每轮都真的去 `onResume()`。`WebView.onResume()` 会走到
> `AwContents.onResume()` 并触发重绘，不是空操作；早先每 500ms 无条件调一次，
> 等于 App 明明在前台也在被**每秒强制刷新 2 次**，真机反馈就是「启动和使用时
> 都变卡了」。WebView 一旦被唤醒会一直跑到下次 `onPause()`，所以每个暂停周期
> 只需要补唤醒一次：`onActivityPaused` 置 `petNeedsResume`，轮询看到标记才唤醒。
>
> 另外**收回后要立刻 `stopKeepAlive()`**：早先让轮询多撑 20 秒是为了「等视口
> 重算」，但那个诊断是错的（真正的病根是 LayoutParams，见下），继续撑着只有
> 性能损失。

进程存活由 `PetForegroundService` 保证，WebView 存活由轮询保证，
这是两个独立问题，缺一不可。

**`FLAG_ACTIVITY_REORDER_TO_FRONT` 会把 singleTask 的 App 直接带崩**

「点 ✕ 收回后要把 App 从后台拉到前台」这条需求，第一版写的是：

```kotlin
activity.startActivity(
    Intent(activity, activity.javaClass)
        .addFlags(FLAG_ACTIVITY_REORDER_TO_FRONT or FLAG_ACTIVITY_SINGLE_TOP)
)
```

真机结果是**点「返回」直接闪退**（App 消失回桌面，属于未捕获异常杀进程）。

本 App 的 `MainActivity` 是 `android:launchMode="singleTask"`
（`gen/android/app/src/main/AndroidManifest.xml`）。singleTask 的启动语义由系统
接管：`ActivityStarter` 会强制补 `NEW_TASK` 并走「复用已有实例 + CLEAR_TOP 式
收尾」那条路径，而 `REORDER_TO_FRONT` 是给**标准**启动模式用的重排标志，
两者语义互斥——这条组合不是文档化用法。

**应对**：改用 Launcher 那条 intent，与用户点桌面图标时系统发出的完全一致：

```kotlin
Intent(Intent.ACTION_MAIN).apply {
    addCategory(Intent.CATEGORY_LAUNCHER)
    component = ComponentName(activity, activity.javaClass)
    addFlags(FLAG_ACTIVITY_NEW_TASK or FLAG_ACTIVITY_RESET_TASK_IF_NEEDED)
}
```

对 singleTask 而言它会复用已有实例、把任务栈移到前台，不会新建实例；
投递的 `onNewIntent` 正是「点图标切回 App」每次都在发生的事，天然安全。
也不需要 `REORDER_TASKS` 权限（`moveTaskToFront` 需要，且更容易被
Android 10+ 的后台启动限制静默拒绝）。

> 教训：Handler 里逃出去的异常会直接杀掉进程。`hide()` 这条路径上的所有
> 延迟任务（`hide` 本体、`pet-attached` 重试、保活轮询）现在都 `catch (t: Throwable)`。

**`setContentView(view)` 不会重置 View 的 LayoutParams**

这条坑了整整四轮真机验证，记在这里。

把 WebView 从 `WindowManager` 搬回 Activity 时，直觉是
`activity.setContentView(view)` 就会让它铺满内容区——**不会**。
实测它保留了悬浮窗那套 `WindowManager.LayoutParams`（展开态 216×252dp），
于是 WebView 回到 Activity 后视图本身还是那么小，**整个 App 被挤在屏幕左上角
一小块里**。

真机诊断数据（360×803dp 屏幕）：

```
[returned]
innerW=216 innerH=252      ← 正是展开态悬浮窗的尺寸
fit=1.000 floating=false   ← 页面侧状态完全正确，问题在原生
```

必须显式改回来：

```kotlin
activity.setContentView(view)
view.layoutParams = ViewGroup.LayoutParams(MATCH_PARENT, MATCH_PARENT)
```

**为什么排查了这么久**：前几轮一直在怀疑「Chromium 的 CSS 视口没重算」，
方向错了。有一次只改了 `lp.height = MATCH_PARENT`（没碰 width），结果表现成
「**高度对了、宽度不对**」——如果真是视口没重算，改高度时宽度会一起更新。
这个「一半好一半不好」的现象本身就反证了根因是 LayoutParams。

> 教训：这类「整个界面缩在一角」的问题，先量 `window.innerWidth`，
> 再和屏宽对照。数字一比就知道是视口问题还是 View 尺寸问题，不用猜。

**「先切页、再搬移」导致页面挂载时还不知道自己在悬浮窗里**

进入流程是刻意排成「① `router.push('/pet')` → ② `showFloatingPet()`」的
（先切页，用户能看到 `/pet` 渲染完成，不会闪一下主界面）。代价是
**`PetMode.onMounted` 跑的时候第②步还没执行**，于是：

```ts
floatingWindowMode.value = isInFloatingWindow(); // ← 必然是 false
```

页面于是掉进**桌面端分支**，真机上就是这一串症状：

| 症状                                   | 原因                                                                           |
| -------------------------------------- | ------------------------------------------------------------------------------ |
| 透明区特别大，且**大小随桌宠缩放变化** | 布局用 `PET_WIDTH_BASE × pet.scale`，而且**没有 `--pet-fit` 整体缩放**         |
| 宠物大小由桌面端设置决定               | `GameRolesStage.frameSize = AVATAR_BAND_BASE × (floatingMode ? 1 : pet.scale)` |
| 渲染出「悬停才浮现」的桌面端按钮       | 桌面端按钮的 `v-if="!floatingMode"`                                            |
| ✕ 走的是桌面端退出路径                 | 那条路**不调用 `hideFloatingPet()`** → 悬浮窗永远留在屏幕上                    |
| 屏幕上的诊断数字一直不出现             | 诊断挂在几何轮询里，而轮询只在「进入悬浮窗」时启动                             |

页面本来可以靠原生的 `pet-detached` 事件自愈，但那条事件本身不可靠
（见 4.4 开头：`evaluateJavascript` 在搬运前后会丢）。

**应对**：手机端 `/pet` 只可能来自悬浮窗流程（不支持/未授权时
`goToPetMode` 会提前 `return`），所以**直接按悬浮窗渲染**，不赌事件：

```ts
if (isAndroid() && !floatingWindowMode.value) enterFloatingLayout();
```

`enterFloatingLayout()` 一次性把形态、`markFloatingWindowMode(true)`、透明背景、
几何轮询都置好；`pollNativeState` 里再用原生的 `status.detached` 兜一层自愈。
`pet/GameRolesStage.vue` 的 `floatingMode` 同理（它只被 PetMode 使用，
所以可以直接 `isInFloatingWindow() || isAndroid()`）。

> 教训：**形态（我在不在悬浮窗里）不能由「原生推来的事件」决定**，只能由
> 「页面自己发起的请求」或「同步可知的平台事实」决定。事件只配当快路径。

## 五、手机端的交互设计

### 5.1 尺寸：固定逻辑画布 + 整体等比缩放

桌面端固定 240×480dp。手机屏宽普遍 360–430dp，直接搬过来会让桌宠占到
**58%–67% 屏宽**（这正是早期「占了手机一半屏幕」的原因）。

但**窗口**按屏幕比例算、**页面**跟着窗口做响应式布局也不行，实测会同时坏三件事：

1. 展开态窗口 2.75 倍宽，头像若锚在收起态宽度，窗口里就是一大片透明区
   ——而 Android 悬浮窗没有逐像素穿透，那片空白还会**吃掉下层 App 的触摸**
2. 输入框、按钮、字号不跟着缩，小窗里挤成一团、点不到
3. 两种形态走不同布局分支，只有一套被真机验证过

因此悬浮窗内改为 **「固定逻辑画布 + 整体等比缩放」**：

- 页面**始终**按桌面端那套 240dp 宽的布局排版（下称逻辑画布），不做响应式
- `#pet-app` 用 `transform: scale(window.innerWidth / 240)` 缩放到窗口大小
- 窗口**宽度**由原生按屏幕比例算（只有原生知道真实屏宽）
- 窗口**高度**由页面按实际内容算好后通过 `set_size` 报回原生
  —— 气泡是流式输出的，高度随时在变，写死常量必然算错

| 状态     | 窗口宽度          | 逻辑高度           | 缩放系数（360dp 屏） | 显示内容      |
| -------- | ----------------- | ------------------ | -------------------- | ------------- |
| 收起     | 1/6 屏宽（60dp）  | 头像带 210         | 0.25                 | 仅头像        |
| 展开     | 0.6 屏宽（216dp） | 头像 210 + 输入 70 | 0.90                 | 头像 + 输入框 |
| 气泡出现 | 同上              | 再加气泡实际高度   | 同上                 | 窗口向下长高  |

展开态取 0.6 屏宽而不是更窄：缩放系数就是「窗口宽度 / 240」，2/5 屏宽
（144dp）时系数只有 0.6，15px 的字缩到 9px 就看不清了；0.6 屏宽时系数 0.9，
字约 13.5px，勉强可读。**这是这个方案的固有代价：文字绝对大小与窗口宽度成正比。**

`MIN_SIZE_DP` 必须够小（现为 24dp）：收起态窗口只有约 60×52dp，
原来的 80dp 下限会把前端报上来的高度硬抬到 80，下方凭空多一块透明区。

### 5.2 手势：拖动 / 点头像 / ✕ 收回

手机没有鼠标，桌面端那套 `mouseenter/mouseleave` 完全不适用：

| 手势       | 行为                                 |
| ---------- | ------------------------------------ |
| 拖动       | 移动窗口，松手后吸附到最近的左右边缘 |
| 单击头像   | 展开 / 收起**来回切换**              |
| 展开后点 ✕ | 收回悬浮窗，切回 App 并跳回聊天页    |

**为什么取消了「双击收回」**：双击和「点头像展开/收起」是**直接冲突**的
——同一位置的两次点按，既可能是「展开 → 收起」，也可能是「收回」，
物理上无法区分。而且原先的判定只看时间不看位置，任意两次 300ms 内的点按
都算双击（点完头像紧接着点输入框也会把桌宠收回去），真机误触严重。

现在收回由展开面板右上角的 **✕** 负责，手势只剩「单击头像 = 展开/收起」
一种，没有歧义。

**✕ 的位置必须在逻辑画布内部**：早先那版挂在头像右上角用
`-top-1 -right-1`（负偏移），整体等比缩放后会被 `#pet-app` 的
`overflow-hidden` 裁掉一半，真机上根本点不到。现在用 `top-1 right-1`，
完整落在画布内。

拖动与点击的区分靠位移阈值（`TAP_SLOP_DP = 16dp`）：没有它，每次拖完都会误触发点击。
取 16dp 而不是 Android 默认的 8dp——这里判定的是「整个窗口要不要跟着手指走」，
不是滚动，手指点按时天然会带几 dp 位移，8dp 会把大量正常点按判成拖动
（表现为「单击经常没反应，窗口还会被带偏一点」）。
一旦判定为拖动，会向 WebView 补发一个 `ACTION_CANCEL`，否则页面那边会一直
停在「按下未抬起」的状态（按钮保持按压态）。

> 那排「悬停才浮现」的桌面端按钮（设置/自动/返回主页/截图/语音）在悬浮窗里
> 一律隐藏：手机没有 hover，且缩放后会被裁掉。

## 六、后续阶段

| 阶段      | 目标            | 关键工作                                          |
| --------- | --------------- | ------------------------------------------------- |
| **P0** ✅ | 技术可行性验证  | 插件骨架、CI 编译通过                             |
| **P1** ✅ | 搬运主 WebView  | 占位页、view 搬运、TCP 保留（IPC/store 完整可用） |
| **P2** ✅ | 手机端交互      | 屏幕比例尺寸、拖动/点头像/✕ 收回、展开收起        |
| **P3** ✅ | 逻辑画布 + 缩放 | 整体等比缩放、内容高度回报、WebView 保活轮询      |
| **P4**    | 气泡与打磨      | 气泡位置策略、权限引导 UI、厂商白名单             |

> 原 P3「气泡悬浮到窗口外」已作废：`FLAG_LAYOUT_NO_LIMITS` 是让**窗口本身**
> 可以超出屏幕边界，**不是**让 WebView 内容画出自己的 bounds。WebView 内容
> 永远被裁剪在自身范围内，气泡不可能靠它跑到窗口外。现在气泡就在窗口内，
> 由窗口跟着长高来容纳（见 5.1）。

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
9. **点右上角 ✕** → 收回悬浮窗，回到 App 主界面（并自动跳到聊天页）

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
| 点头像展开/收起           | ✅                                            |
| 点 ✕ 收回并跳回聊天页     | ✅ 依赖 `pet-attached` 事件（见 7.5）         |
| 退后台后继续运行          | ✅ 前台服务保进程 + 500ms 轮询保 WebView      |
| 气泡在窗口内随内容长高    | ✅ 窗口高度由页面回报给原生                   |

### 7.4 权限被拒 / 找不到开关

- **国产 ROM**：小米/华为/OPPO 等除「显示在其他应用上层」，还需在
  「后台弹出界面」「自启动」里放行，否则悬浮窗被静默拦截
- **没弹设置页**：部分系统该权限默认关闭且无入口，属系统限制
- **排查**：看 logcat 中 `FloatingPet` 标签的输出

### 7.5 已知风险点（真机重点观察）

| 现象                                 | 可能原因                                                                                                                                                               |
| ------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 切回 App 白屏                        | 占位页没生效，`setContentView` 顺序有问题                                                                                                                              |
| 收回后主界面黑屏                     | WebView 搬回失败，需看 `restoreWebViewToActivity` 日志                                                                                                                 |
| **收回后只剩左上一角、不回聊天页**   | `pet-attached` 没送达。三层保险：`notifyWeb` 显式收 WebView 参数、0/300/1000/3000ms 重试、Activity resume 时由 `ensureLifecycleCallbacks` 补发                         |
| **收回后整个界面缩在左上角**         | **`setContentView` 没重置 LayoutParams**（见 4.4）。先量 `window.innerWidth` 和屏宽对照：等于悬浮窗宽度就是这条，等于屏宽则是视口问题                                  |
| **点 ✕ 收回直接闪退**                | `bringActivityToFront` 用了 `FLAG_ACTIVITY_REORDER_TO_FRONT` + singleTask（见 4.4）；现在改成 Launcher intent                                                          |
| **收回后整个 App 用窄视口渲染**      | 先量 `window.innerWidth`。等于屏宽就是视口问题（收回后 `forceViewportRefresh` 强制重算）；等于悬浮窗宽度则是 LayoutParams                                              |
| **展开后一大片透明区、吃掉下层触摸** | 悬浮窗是矩形、宠物是圆形，透明角落**无法**逐像素穿透（见 4.4）。先看诊断里 `band=`：不为 0 说明气泡带占了高但没渲染；再看 `role scaleP=`：< 1 说明宠物只占头像框一部分 |
| **退后台后桌宠不动**                 | 保活轮询没起来；看 logcat 里 `FloatingPet` 的「已启动保活轮询」                                                                                                        |
| **悬浮窗里点不到按钮**               | 按钮落在缩放后的逻辑画布外，被 `#pet-app` 的 `overflow-hidden` 裁掉                                                                                                    |
| **气泡看不到 / 以为消息没发出**      | 窗口高度没跟着内容长，气泡被裁；查 `reportFloatingHeight` 的 IPC 是否成功                                                                                              |
| **单击经常没反应、窗口被带偏**       | `TAP_SLOP_DP` 偏小，正常点按被判成拖动                                                                                                                                 |
| 悬浮窗里输入框弹不出键盘             | 窗口 `FLAG_NOT_FOCUSABLE` 没在展开态摘掉                                                                                                                               |
| 按住 Home 后悬浮窗消失               | 前台服务被 ROM 拦截，需加「后台弹出界面」白名单                                                                                                                        |

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
