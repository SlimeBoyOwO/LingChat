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
  ⑤ 原生 moveTaskToBack(true)         ← 延后 400ms，把 App 自己退到后台

收回：
  ⑥ 原生 moveTaskToFront(taskId)      ← 先把任务栈拉回前台
  ⑦ 原生 removeView(webView)
  ⑧ 原生 setContentView(webView)      ← 主界面原样回来
  ⑨ 前端 router.push('/chat')
```

**顺序不能反**：先搬移再切页的话，用户会看到主界面闪一下才变成桌宠。

#### 4.3.1 为什么第 ⑤ 步要把 App 自己退到后台

桌宠要浮在**桌面 / 其他应用**之上，App 自己必须先让开。主题里虽然配了
`windowIsTranslucent` + `windowShowWallpaper`（窗口透明、显示系统壁纸），
但那只是「窗口透明」——只要 App 还留在前台，它就**占着整块屏幕**：用户看不到
自己的桌面，也点不到别的应用图标，那整屏区域仍然归我们的窗口所有。

退到后台后：壁纸/桌面立刻可见，桌宠浮在上面；用户从最近任务切回来时看到的是
占位页上那两行引导文案。WebView 此刻已经在 `WindowManager` 里（不随 Activity
进后台），加上 `PetForegroundService` 的前台优先级，渲染与 IPC 都不受影响。

**三个容易踩的点：**

| 点                                                     | 说明                                                                                                                                                                                                                                                                                                                                 |
| ------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **必须延后 400ms（`BACKGROUND_DELAY_MS`）**            | `pet-detached` / `pet-metrics` 是 `evaluateJavascript` 异步投递的，而 WebView 一旦因宿主 Activity 进后台被 `onPause`，**未执行的那几条会被整体丢弃** —— 页面就永远停在桌面端布局里。等它们落地再退                                                                                                                                   |
| **前台服务必须先起来**                                 | `PetForegroundService.start(activity)` 是在 App 还在前台时调用的（顺序在 `moveTaskToBack` 之前）。Android 14+ 对后台启动前台服务有限制，顺序反了会起不来，退后台后进程就随时可能被回收、悬浮窗直接消失                                                                                                                               |
| **收回要靠 `moveTaskToFront`，不能用 `startActivity`** | `MainActivity` 是 `singleTask`，`startActivity` 会给已有实例投递 `onNewIntent`，与「正在搬运 WebView」叠在一起会闪退。`moveTaskToFront(taskId, MOVE_TASK_NO_USER_ACTION)` 走任务栈、不构造 Intent，需要 `REORDER_TASKS`（normal 级，声明即授予）。另外本 App 持有 `SYSTEM_ALERT_WINDOW`，在 Android 10+ 的后台启动限制里属于豁免情形 |

> 这一版之前 App 一直留在前台，所以 `moveTaskToFront` 实际是**空操作**，
> 那条「点 ✕ 收回」的路径从未真正被走过。加上退后台之后它**第一次真正生效**，
> 因此「点 ✕ 收回」需要重新真机验证（这也是为什么收回侧备了三层
> `pet-attached` 补发保险，见 8.5）。

#### 4.3.2 占位页必须刷**不透明**底色 —— 一个被推翻的结论

`buildPlaceholderView()` 里的第一行是：

```kotlin
val root = android.widget.FrameLayout(context)
root.setBackgroundColor(Color.parseColor("#101014"))   // 不透明的近黑
```

这行**被删过一次，又被加了回来**，两次的理由都是真机现象，但第一次的归因是错的。
记在这里，免得下次又有人看着「窗口透明」这四个字把它删掉：

| 轮次 | 观察到的现象                                                            | 当时的归因                                                       | 处理         |
| ---- | ----------------------------------------------------------------------- | ---------------------------------------------------------------- | ------------ |
| 早先 | 桌宠悬浮时，屏幕上有一大块「外面很大一片区域」                          | 认为占位页那层不透明底色把主题配好的透明窗口 + 壁纸盖住了        | **删掉底色** |
| 现在 | 切回 App 时占位页那两行引导文案**直接压在壁纸上**，深色壁纸下几乎看不清 | 那片区域的真身是悬浮窗缺 `FLAG_NOT_TOUCH_MODAL` 时吃掉的整屏触摸 | **加回底色** |

关键点：**那片区域的成因与占位页底色无关**。`FLAG_NOT_TOUCH_MODAL` 的缺失会让
悬浮窗把**整块屏幕**的触摸都吃掉（见 5.1.4），现象上看起来就是「App 外面多了一大片」，
而占位页此刻根本不在屏幕上（App 已经 `moveTaskToBack` 退到后台了）。

所以占位页该按它的**真实使用场景**设计：它只在**用户主动切回 App** 时才可见，
需要的是一块读得清字的背板，而不是「透出壁纸」。

主题里的 `windowIsTranslucent` / `windowShowWallpaper` **保持不动** —— 它们对
占位页之外的行为（进入 `/pet` 那一瞬间的过渡）仍然有意义，和这行底色不冲突：
`setBackgroundColor` 设的是**内容视图**的底色，`windowBackground` 设的是**窗口**的。

### 4.4 两个必须知道的平台限制

**点击穿透是窗口级开关，无法按区域精细控制**

桌面端（Windows）用 `GetCursorPos` 轮询 + `set_ignore_cursor_events`，可以按
**像素区域**判定是否穿透。Android 的 `FLAG_NOT_TOUCHABLE` 只能作用于**整个窗口**，
而且它在**按下那一刻**就被求值——没有「先看手指落在哪再决定穿不穿透」的余地。

**应对**：窗口尺寸按屏幕比例收窄（收起态仅 1/6 屏宽），不做大块透明留白。

> ⚠️ **这里曾经写着一个错误的「残留问题（未解决）」**，说「展开态必然有一圈
> 透明角落吃触摸，是矩形窗口 + 圆形宠物的固有代价」。
>
> **那是错的，而且把排查带偏了两轮。** 「一整屏都能摸、摸着还能把桌宠拖走」
> 跟窗口形状毫无关系 —— 它是**少了一个 `FLAG_NOT_TOUCH_MODAL`**：
> 没有这个 flag 的窗口，可触摸区域不是自己那块矩形，而是**整块屏幕**。
> 详见 **5.1.4**，那里有官方定义原文和完整推导。
>
> 真正的固有代价比这个小得多：只有**窗口矩形以内、宠物轮廓以外**的那几个
> 角落吃触摸（收起态 60×60dp、展开态 216×252dp 之内）。窗口之外的一切
> 触摸都会正常落到下层应用上。
>
> 真机上先看诊断里的两组数字：
>
> - `band=`（气泡带 `offsetHeight`）：不为 0 说明气泡带占了高度却没渲染出内容，
>   会在宠物与输入框**中间**留出一条透明带
> - `role scaleP=`：角色卡的「桌宠缩放」。它 < 1 时宠物只占头像框的一部分，
>   四周全是透明区。桌面端靠点击穿透忽略这些区域，Android 上则会吃掉触摸
>
> 诊断代码在 `PetMode.vue` 的 `DEBUG_FLOATING_OVERLAY` 段落，**合并前必须删除**。

**「收起态正常、展开态不正常」的唯一结构性差异：`FLAG_NOT_FOCUSABLE`**

这条值得单独记，因为它一句话解释了一整类症状，也是排查时最容易被忽略的分水岭。

```kotlin
if (expanded) {
    params.flags = params.flags and FLAG_NOT_FOCUSABLE.inv()   // ← 变成「可获焦」
    params.softInputMode = SOFT_INPUT_ADJUST_RESIZE
} else {
    params.flags = params.flags or FLAG_NOT_FOCUSABLE          // ← 一直「不可获焦」
}
```

- **收起态**：窗口 `FLAG_NOT_FOCUSABLE`。系统**根本不会**给它算 window insets，
  也不会为它处理输入法 —— 内容区永远是我们请求的那个尺寸，画布严丝合缝。
- **展开态**：摘掉 `FLAG_NOT_FOCUSABLE` 之后，窗口被拖进系统的 insets / IME 机制。
  系统会按自己的规则**改写内容区大小**：状态栏/导航栏/挖孔作为 padding 加进来、
  `ADJUST_RESIZE` 又会在输入法弹出时压缩高度。

而插件侧**一个 insets 都没处理过**（`fitInsetsTypes` / `setDecorFitsSystemWindows`
在修复前一次都没出现）。更关键的是：**画布的尺寸只由窗口「宽度」推出**
（`--pet-fit = 窗口宽 / 240`，高度恒为 `画布逻辑高 × fit`）—— 系统改**高度**，
页面完全不知情。于是画布比视口高 → 溢出 → 浏览器把内容往上顶 →
宠物顶部被切掉、下方空出一片。用户看到的就是「大片空白」。

**修法（三处，缺一不可）**：

1. 去掉 `SOFT_INPUT_STATE_VISIBLE`。它会让**一点展开输入法就自动弹出**，
   瞬间触发上面那条链。只留 `ADJUST_RESIZE`，键盘仍然会在用户**主动点输入框**时
   正常弹出（窗口此时已可获焦），但不会在展开的一瞬间被系统强行改尺寸。
2. `fitInsetsTypes = 0`（API 30+）/ 等价的 `systemUiVisibility` 标志，
   明确声明「本窗口自己处理 insets」。见 `show()`。
3. 页面侧兜底：缩放系数同时受**宽和高**约束 ——
   `fit = min(视口宽/240, 视口高/画布逻辑高)`。窗口尺寸正常时两项相等，
   是恒等变换；视口被系统压矮时则自动缩小内容，保证内容完整可见。
   见 `PetMode.vue` 的 `fitForViewport()`。

> **教训**：凡是「窗口某个属性只在某一形态下改变」的地方，都要问一句
> 「这个属性会不会把窗口拖进某个我们没实现的系统机制」。`FLAG_NOT_FOCUSABLE`
> 就是这样一个开关——它看起来只关乎「能不能输入」，实际上还决定了
> **系统会不会插手窗口的内容区尺寸**。

**尺寸基准必须取屏幕短边，否则横屏必崩**

`collapsedSize()` / `expandedSize()` 早先用 `screenWidthDp()` 当基准：

```
横屏 802×360 dp，展开态宽 = 802 × 0.6 = 481dp
  高 = 481 × (280/240) = 561dp  >  屏幕高 360dp
```

窗口比屏幕还高 201dp，`clampIntoScreen()` 只能把它按到 `y=0`，
宠物下半身和整个输入带被挤到屏幕外。整套尺寸只按竖屏设计过。

改成 `screenBasisDp() = min(屏宽, 屏高)` 之后：

| 方向         | 短边 | 展开态窗口 |
| ------------ | ---- | ---------- |
| 竖屏 360×802 | 360  | 216×252    |
| 横屏 802×360 | 360  | 216×252    |

竖屏结果与旧行为**逐位相同**（无回归），横屏也稳稳落在屏幕里。
副产品：桌宠的**物理尺寸与方向无关**，转屏时不会突然变大变小。

**`resize` 监听曾经是一条死通道**

`onMounted` 里那句 `window.addEventListener("resize", onFloatingResize)`
只在「挂载时已经是悬浮窗」的分支里执行。而按本页的启动顺序
（`goToPetMode` 是「先 `router.push('/pet')`，再 `showFloatingPet()`」），
挂载那一刻**必然还不是**悬浮窗 —— 也就是说那条分支**永远走不到**。

后果：窗口尺寸变化时页面收不到任何通知，只剩原生 `evaluateJavascript`
推 `pet-metrics` 这一条路；而那条路在搬运/收回前后并不可靠。
修法：在 `enterFloatingLayout()` 里补绑（`addEventListener` 对同一函数引用幂等）。

> 在 `resize` 回调里读 `window.innerWidth/Height` 是**安全**的：这个事件的
> 定义就是「视口尺寸刚刚变了」，此刻读到的就是新值。滞后问题只存在于
> 「改完布局立刻读」的场合（原生 `updateViewLayout()` 之后），不是这里。

**怎么用截图定位几何问题（省掉反复试错）**

诊断浮层会实时给 `#pet-app` 铺一层半透明品红、给各条带描边。拿到截图后
**不要靠肉眼看**，直接按颜色提取像素：

```python
# 品红 #ff00ff = 画布，绿 #4ade80 = 输入带，青 #22d3ee = 头像带
pts = [(x, y) for y in range(H) for x in range(W) if near(px[x, y], (255, 0, 255))]
```

用列/行计数找出长直线，就能拿到画布四边的**物理像素**坐标；再除以
`devicePixelRatio` 换成 CSS px，和 `window.innerWidth` 一比：

- 画布宽 ÷ 210（`FLOATING_LOGICAL_WIDTH`）= 实际生效的 `--pet-fit`（和诊断里 `applied=` 对照）
- 画布左边 / 上边 ≠ 0 → **画布被顶偏了**（页面被滚上去 / 系统平移了视口）
- 画布右边 / 下边有余量 → **窗口比画布大**

> ⚠️ 诊断浮层的文字必须**实时刷新**（挂在轮询上）。早先它只在
> `enter` / `returned` 那一刻画一次、30 秒内不再更新，于是**文字是那一刻的快照，
> 而描边是实时的** —— 两者对不上，排查时被误导过整整一轮
> （文字写着 `fit=1.000`，实际早就变成别的值了）。
> 另外 `floatingFit.value` 可能比 DOM 新：Vue 改了 ref 但还没 flush，
> 此时读 `getBoundingClientRect()` 得到的是**旧**系数算出的矩形。
> 所以诊断里额外用 `getComputedStyle(el).transform` 读矩阵，
> 拿到的是**真正生效**的缩放系数。

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

必须显式改回来。但**写法很关键**：

```kotlin
// ✅ 正确：走 setContentView 的两参重载，由 addViewInner 把参数归一成
//    FrameLayout.LayoutParams(MATCH_PARENT, MATCH_PARENT)
activity.setContentView(
    view,
    ViewGroup.LayoutParams(MATCH_PARENT, MATCH_PARENT)
)

// ❌ 错误：setContentView 之后**事后**赋值
activity.setContentView(view)
view.layoutParams = ViewGroup.LayoutParams(MATCH_PARENT, MATCH_PARENT)
```

**为什么第二行会闪退**：`setContentView(view)` 内部是
`contentParent.addView(view)`，而 contentParent 是 `FrameLayout`
（AppCompat 下是 `ContentFrameLayout`）。`addViewInner` 会把 View 的
LayoutParams 归一成 `FrameLayout.LayoutParams`。事后塞一个**基类**
`ViewGroup.LayoutParams` 进去，下一次 measure 时
`FrameLayout.onMeasure` 里的

```java
final LayoutParams lp = (LayoutParams) child.getLayoutParams();
```

会抛 **`ClassCastException`**。这是主线程未捕获异常 → 直接杀进程。

> **这就是「点 ✕ 收回就闪退」的真凶**：它从第 50 轮（`82ef290b` 引入这行）
> 起一直存在。后面几轮改的 `startActivity` flag、`moveTaskToFront`、
> 删 `forceViewportRefresh` 全都不是病根——它们改的是**别的**嫌疑点，
> 所以「修了还是闪退」。
>
> 可验证的推论：CCE 发生在**下一帧的 measure**，不在赋值那一行。因此
> logcat 里异常栈顶应是 `FrameLayout.onMeasure` 抛 `ClassCastException`，
> 且崩溃前画面已经切回主界面（不是黑屏/白屏）。若实测与此不符，
> 说明收回路径上还有第二个病根，别急着收工。

**为什么排查了这么久**：前几轮一直在怀疑「Chromium 的 CSS 视口没重算」，
方向错了。有一次只改了 `lp.height = MATCH_PARENT`（没碰 width），结果表现成
「**高度对了、宽度不对**」——如果真是视口没重算，改高度时宽度会一起更新。
这个「一半好一半不好」的现象本身就反证了根因是 LayoutParams。

> 教训：这类「整个界面缩在一角」的问题，先量 `window.innerWidth`，
> 再和屏宽对照。数字一比就知道是视口问题还是 View 尺寸问题，不用猜。

**`view.width` 在 `updateViewLayout()` 之后是「上一形态」的值**

这是「展开后一大片空白」的直接成因。

`updateViewLayout()` 是**异步**的：它只把「重新布局」排进下一帧，
返回时 `view.width` 仍是**上一次**布局的结果。而 `show` / `setExpanded` /
`setSize` 都在 `updateViewLayout()` 之后**立刻**把几何推给页面
（`notifyMetrics`），于是推出去的是上一形态的宽度：

```
点展开：
  原生  窗口 60dp → 216dp，updateViewLayout()
  原生  notifyMetrics() → view.width 还是 60dp 对应的 180px
        → 推给页面 scale = 180/3/240 = 0.25（收起态的系数！）
  页面  floatingFit = 0.25 → 240dp 的画布只渲染成 60dp 宽
  页面  reportFloatingHeight() → 用 0.25 算出高度 70dp
        → setSize(0, 70) 把刚展开的窗口高度也改塌
  结果  窗口 216×70，内容 60×70 挤在左上角 —— 用户看到的就是
        「大片空白，但每个框都没问题」
```

**修法（两处，缺一不可）**：

1. `currentScale()` / `status()` 一律用 `params.width`（我们写进去的请求值，
   永不过期）而不是 `view.width`。见 `authoritativeWidthPx`。
   并在布局落定后再用实测 `view.width` 补推一次（`notifyMetrics` 的第二拍）。
2. 页面回报高度时改传**逻辑高度**（`SizeArgs.logicalHeight`），由原生按
   自己手里的权威宽度换算实际高度。这样「窗口高度 ≡ 逻辑高度 × 窗口宽度 / 240」
   由原生**在构造上**保证，页面手里那个系数过期与否都不再影响窗口尺寸。

> 教训：**凡是「改完布局立刻读回尺寸」的写法，都要先问一句「这个读数是
> 这一次布局的吗」**。Android 的 View 尺寸、WebView 的视口，在
> `updateViewLayout` / `addView` 之后都是滞后的。宁可让「权威值」跟着
> 我们自己写进去的 `params` 走。

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

> ⚠️ 上面这段里的「应对」已经**过时**：后来实测「抢跑成悬浮窗形态」会让页面
> 在搬移完成前把宠物画布渲染在**整屏** WebView 里（诊断实测
> `inner=360x802 floating=true canvas=360x315@0,0`，屏幕下方空出四百多 dp）。
> 现行做法是两条路都不走：形态一律等 `pollNativeState` 问出 `status.detached`
> 再切（见 `enterFloatingLayout` 的注释）。

#### 4.4.1 `fit` 的三个来源都会失效 —— 必须有「不依赖任何原生通道」的自愈

悬浮窗里画布永远是 `FLOATING_LOGICAL_WIDTH`(=210) 逻辑宽，靠
`transform: scale(fit)` 铺满窗口。
**`fit` 只要不是 `窗口宽/210`，窗口里就一定出现透明带**（画布比窗口窄）。
而 `fit` 只有三个来源，且**可以同时失效**：

| 来源                                         | 失效场景                                             |
| -------------------------------------------- | ---------------------------------------------------- |
| 原生推 `pet-metrics`（`evaluateJavascript`） | WebView 刚重挂、宿主 Activity 在后台时被整体丢弃     |
| 原生轮询 `status`（500ms IPC）               | 任一环节失败，`fit` 就永远停在初值 1.0               |
| `resize` 事件                                | 只在视口尺寸**变化**时才有；视口没变过就一次都不派发 |

三者同时失效是**真机抓到过的**（`Screenshot_20261001_150746.jpg`）：

```
[enter] inner=360x802 dpr=3
win=0x0dp fit=1.000 applied=1.00        ← IPC 一次都没成功，fit 停在初值
app=360x803@0,0
canvas=240x480@0,0                      ← 画布只有 240 宽，视口却有 360
gap L0 T0 R120 B322 <== 空白!           ← 360-240=120，803-480=323
```

于是**新增 500ms 本地心跳** `startSelfHeal()`：它只读 `window.innerWidth`、
不碰 IPC，系数不对就重算并回报高度。只要页面还在跑，画布就一定会铺满视口宽度。

**同时 `fitForViewport()` 不再对高度取 `min`。** 早先写的是
`min(innerW/240, innerH/逻辑高)`，本意是防「系统把视口压矮、内容溢出被顶到上方」，
但它把系数压小的同时**画布宽度也跟着小于窗口宽度** → 右侧和下侧各留一条透明带。
而画布视觉高度 `逻辑高 × fit` 与窗口高度（原生按同一个 `逻辑高 × fit` 反算，
见 `setSize` 的 `logicalHeight` 口径）是**构造一致**的，页面不该再约束一次。

> 那条透明带**照样吃触摸**：它落在 `#pet-app` 之外、窗口之内，手指点上去会派发
> `mouseleave`，触发桌面端的「光标离开即收起输入框」。用户报的
> 「展开后按空白区域会触发输入框折叠」就是这么来的 —— 它是空白存在的**旁证**，
> 不是独立 bug。顺带把 `handleMouseEnter/Leave` 在悬浮窗里改成空操作。

#### 4.4.2 旋转后桌宠跑到屏幕外 —— 没有任何回调会处理它

`MainActivity` 的 manifest 声明了：

```xml
android:configChanges="orientation|keyboardHidden|keyboard|screenSize|locale|smallestScreenSize|screenLayout|uiMode"
```

所以旋转**不会重建 Activity**。这对悬浮窗是好事（插件实例、WebView、窗口全活着），
但也意味着**没有任何人**会处理旋转之后的布局：

- 悬浮窗的 `x` / `y` 是**屏幕坐标系里的绝对值**，而旋转会把屏幕宽高**对调**。
  竖屏 360×802 里 `y = 700`（贴着屏幕下沿）的桌宠，转到横屏 802×360 之后
  `y` 仍然是 700 > 360 —— 整个窗口飞到屏幕外。
- 尺寸基准是屏幕**短边**（见 4.4 的 `screenBasisDp`）。竖屏 360×802 与横屏
  802×360 的短边都是 360，通常不变；但分屏 / 折叠屏展开会变。

**修法**：注册**显示器回调**（`ensureDisplayListener`），在 `onDisplayChanged` 里延后
`CONFIG_SETTLE_MS`（120ms）调 `reapplyWindowAfterConfigChange()`：

1. 按当前展开态重算尺寸；
2. **左右保留原来那一边**（竖屏贴右沿的，转横屏后还在右沿）；
3. **上下保留相对位置** `y / (屏高 − 窗高)`（竖屏贴下沿的，转横屏后仍在下方）；
4. `clampIntoScreen` → `updateViewLayout` → `notifyMetrics`（窗口宽度可能变了，
   必须把新系数推给页面）。

「原来贴哪一边」靠字段 `lastScreenW/H` 判断，它在 `clampIntoScreen()` 与
`show()` 里记录 —— `clampIntoScreen` 是所有布局路径的必经之地，是最可靠的记录点。

> ⚠️ 这里最初用的是 `ComponentCallbacks.onConfigurationChanged`，**它在悬浮窗场景里
> 永远不会被调用** —— 见 4.4.2.3。

##### 4.4.2.1 屏幕尺寸**不能**经过 App 的 Resources —— 三个来源都被证伪过

真机现象一：**横屏时桌宠只能停在左半边** —— 往右拖到大约「竖屏宽度」的位置就停住，
右边一大片过不去。
真机现象二：**旋转屏幕后桌宠跑出屏幕**，转多少次都回不来。

两者是**同一个成因**：读到的屏幕尺寸一直停在竖屏。

三版读数来源，一版比一版像对的：

| 版本    | 来源                                                             | 结果                                      |
| ------- | ---------------------------------------------------------------- | ----------------------------------------- |
| 第 1 版 | `activity.resources.displayMetrics`                              | ❌ 现象依旧                               |
| 第 2 版 | `activity.getSystemService(WINDOW_SERVICE).maximumWindowMetrics` | ❌ **现象依旧**（同一个 Resources）       |
| 第 3 版 | `DisplayManager.getDisplay().getRealMetrics()`                   | ❌ **现象依旧**（又绕回同一个 Resources） |

**第 2 版**看着像「窗口服务」的权威读数，但 AOSP 的实现
（`android/window/WindowMetricsController.java`）是：

```java
private WindowMetrics getWindowMetricsInternal(boolean isMaximum) {
    final Configuration config = mContext.getResources().getConfiguration();
    final WindowConfiguration winConfig = config.windowConfiguration;
    bounds = (isMaximum) ? winConfig.getMaxBounds() : winConfig.getBounds();
    ...
}
```

—— **读的还是同一个 Resources**，只是把 `displayMetrics` 换成了
`windowConfiguration.maxBounds`。所以第 2 版等于没改。

**第 3 版**看着终于离开 Resources 了，其实**又绕了回去**
（`android/view/Display.java`）：

```java
public void getRealMetrics(DisplayMetrics outMetrics) {
    ...
    mDisplayInfo.getLogicalMetrics(outMetrics, ...);   // ← 这里已经是当前旋转的尺寸
    final int rotation = getLocalRotation();           // ← 读的却是 mResources 的配置
    if (rotation != mDisplayInfo.rotation) {
        adjustMetrics(outMetrics, mDisplayInfo.rotation, rotation);   // ← 又交换回竖屏
    }
}
```

`mResources` 就是这个 `Display` 关联的 Resources —— `DisplayManager` 是
`activity.getSystemService(DISPLAY_SERVICE)` 拿到的，`getDisplay(id)` 内部走
`mGlobal.getCompatibleDisplay(id, mContext.getResources())`，所以它**就是 Activity 的**。
悬浮窗里 Activity 在后台，它的旋转停在「进入悬浮窗那一刻」，于是**已经正确的
横屏尺寸被 `adjustMetrics` 又交换回竖屏**。

> `shouldReportMaxBounds()` 为真时更直接：走 `getMaxBoundsMetrics`，那本来就是拿
> `mResources.getConfiguration()` 算的。

**为什么这个 Resources 在悬浮窗场景里必然不准**：桌宠浮在桌面上时，宿主 Activity
已经被 `moveTaskToBack(true)` 退到后台（见 4.3.1）。**系统只对可见 Activity 派发
配置变化**，后台 Activity 的 Resources 配置不保证跟随此后发生的旋转刷新。于是屏宽
永远停在「进入悬浮窗那一刻」的竖屏值：

- 边界偏小 → 现象一：永远拖不过「竖屏宽度」那条看不见的线；
- `lastScreenW/H` 也永远不变 → 旋转重排永不触发 → 现象二。

同一个原因还让 `onConfigurationChanged` 收不到（见 4.4.2.3）。

**正确来源：两个直读 `DisplayInfo` 的读数。**

```kotlin
private fun screenSizePx(): Pair<Int, Int> {
    try {
        val display = displayManager().getDisplay(Display.DEFAULT_DISPLAY)
        if (display != null) {
            val mode = display.mode
            val pw = mode?.physicalWidth ?: 0
            val ph = mode?.physicalHeight ?: 0
            if (pw > 0 && ph > 0) {
                // 物理分辨率**不随旋转**，所以要用 rotation 自己换宽高
                val rotated = display.rotation == Surface.ROTATION_90 ||
                    display.rotation == Surface.ROTATION_270
                return if (rotated) ph to pw else pw to ph
            }
        }
    } catch (t: Throwable) {
        Log.w(TAG, "读取屏幕尺寸失败，退回 resources.displayMetrics", t)
    }
    val dm = activity.resources.displayMetrics      // 兜底：宁可边界偏小，也不能抛
    return dm.widthPixels to dm.heightPixels
}
```

`Display.getRotation()` 与 `Display.getMode()` 都只读 `mDisplayInfo` 自己的字段，
**不经过任何 Resources / DisplayAdjustments**：

- `getRotation()` → `mDisplayInfo.rotation`，物理旋转，实时；
- `getMode()` → 物理分辨率，不随旋转，所以要用 rotation 自己换宽高。

**判据**：只要一个 API 的签名里出现 `Resources` / `Configuration` /
`DisplayAdjustments`，或者内部会读它们，在这个场景下就**不可信**。
`getRealMetrics` / `getSize` / `getMetrics` / `WindowMetrics` 全都中招 ——
前两个看着在 `Display` 上，实则内部都要过一次 `adjustMetrics`。

所有读屏幕尺寸的路径都收敛到这一个函数（`screenWidthDp` / `screenHeightDp` /
`clampIntoScreen` / `snapToEdge` / `reapplyWindowAfterConfigChange` / `show()` /
`keepAliveTick`），改一处即全部生效。

> **转屏后位置不对时先看日志**：
> `屏幕尺寸变化：悬浮窗重排为 … 屏幕 WxH（rotation=…, mode=…x…）` ——
> rotation 与物理尺寸都在，一眼能分清是「旋转没读到」还是「读到但算错了」。

> `density` 仍然取自 `activity.resources.displayMetrics.density` —— 密度不受旋转影响，
> 而且 WebView 的 `devicePixelRatio` 也来自它，两边必须同源。

##### 4.4.2.2 旋转还有一条**不依赖系统回调**的兜底

`onDisplayChanged` 是快路径（约 120ms），但本项目已经反复踩到「系统回调不保证送达」
（`pet-detached` / `pet-attached` 都丢过）。那条回调一旦丢掉，桌宠会一直停在旧屏幕的
坐标系里 —— 竖屏贴下沿的 `y` 在横屏里远大于屏高，整个窗口跑到屏幕外，而用户没有任何
办法把它拉回来。

因此在已有的 500ms 保活轮询（`keepAliveTick`）里加了一道判据。两个调用点（显示器回调
与轮询）共用同一个 `maybeReapplyForScreenChange()`：

```kotlin
private fun maybeReapplyForScreenChange(): Boolean {
    if (!petDetached) return false
    val (curW, curH) = screenSizePx()
    val changed = lastScreenW > 0 && lastScreenH > 0 &&
        (curW != lastScreenW || curH != lastScreenH)
    if (changed) reapplyWindowAfterConfigChange()
    return changed
}
```

`lastScreenW/H` 表示「当前 `x` / `y` 是按哪块屏幕算出来的」，两者与实时读数不一致
== 屏幕变过、但窗口还没跟着重排。代价是每 500ms 一次 `screenSizePx()` 读数
（本地调用，可忽略）。

**这个尺寸比较不是可选的**：`reapplyWindowAfterConfigChange()` 会把桌宠**重新吸附到
边缘**，而 `onDisplayChanged` 不只在旋转时触发（亮度、刷新率、分辨率变化都会来）。
无条件重排会在用户正拖着桌宠时把它直接拽走。

轮询里这个函数的返回值还有第二个用途：**没变才**顺手 `notifyMetrics` 重推一次窗口
几何（让页面在半秒内自愈滞后的视口），变了就由重排路径自己推。

这条兜底同时覆盖两个方向：**横屏 → 竖屏**（原来那条 bug）和 **竖屏 → 横屏**
（用户补充反馈的方向）。

##### 4.4.2.3 回调必须是 `DisplayManager` 的，不能是 `ComponentCallbacks`

`ComponentCallbacks.onConfigurationChanged` **只在 Activity 可见时才送达**。而桌宠浮在
桌面上时，宿主 Activity 已经被 `moveTaskToBack(true)` 退到后台 —— 恰恰是最需要它的那个
场景，它**永远不会来**。这是「旋转后桌宠跑出屏幕」一直修不掉的第二个原因（第一个是
读数取错了源，见 4.4.2.1）。

`DisplayManager.registerDisplayListener` 走的是**进程级**的显示器回调，不看 Activity
可不可见，只要进程活着就会收到（悬浮窗有前台服务保活）。

旋转时这个回调会连着来好几次，且显示状态未必已经落定，因此合成一拍：每次触发先
`keepAliveHandler.removeCallbacks(reapplyRunnable)` 撤掉上一次的待办，只按最终尺寸
重排一次（延后 `CONFIG_SETTLE_MS` = 120ms）。

##### 4.4.2.4 反复进出悬浮窗会**卡成聊天页** —— 误收回是三条路

真机现象（用户反馈）：**反复切来切去时，悬浮窗偶尔变成聊天页，角色凭空消失。**
不好复现，但频率不低。

两个现象是**同一个 bug 的两种终态**，都出自 `PetMode.handleReturnedToApp()` 被误触发：

| 终态                     | 触发条件                                                          |
| ------------------------ | ----------------------------------------------------------------- |
| 悬浮窗里变成聊天页       | `router.push('/chat')` 成功                                       |
| 角色凭空消失（窗口还在） | push 没落地，页面留在 `/pet` 却按**桌面端布局**渲染进那个小窗口里 |

后者尤其隐蔽：`floatingWindowMode` 被置成 false 之后，页面会按 `PET_WIDTH_BASE`(240)
那套桌面端尺寸排版、`--pet-fit` 归 1，而窗口只有约 60dp 宽 —— 用户看到的就是
「悬浮窗还在、里面空了」。

**为什么不可逆**：`handleReturnedToApp()` 里除了 push 路由，还会
`stopMetricsPolling()`。一旦误判，500ms 轮询这条唯一的自愈通道就断了，
页面再也回不到悬浮窗形态。

**三条误判路径**（都要求先 `sawDetached === true`，也就是「进过一次悬浮窗」）：

| #   | 路径                              | 机制                                                                                                                                                                                            |
| --- | --------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| ①   | 轮询读到 IPC 失败的**占位值**     | `getFloatingPetStatus()` 的 `catch` 直接返回 `detached: false`，而调用方把它读作「用户已收回」。搬运 / 旋转重排期间主线程忙着布局，正是查询最容易失败的时候 —— 也就是「反复切来切去」的时候     |
| ②   | 迟到的 `pet-attached`（**主因**） | 收回时原生会重发 **4 次** `pet-attached`（`PET_ATTACHED_RETRY_DELAYS_MS` = 0/300/1000/**3000**ms）。用户在 3 秒内重新进悬浮窗，最后那条就会落在**新一轮**的监听器上，把「刚进来」判成「已收回」 |
| ③   | `pendingAttachNotify` 跨轮残留    | `show()` 里没有清这个引用，收回时记下的 WebView 会一直挂到 20 秒后或 Activity resume 时补发 —— 同样会落进新一轮                                                                                 |

**修法**（原生 2 处 + 前端 3 处）：

1. **原生 `petModeEpoch`**：`show()` / `restoreWebViewToActivity()` 各递增一次。
   收回时排队的每个延迟任务在**入队时**记下当时的轮次，执行前比对 —— 轮次变了
   就说明期间已经重新进出过，这条事件属于上一轮，直接丢弃。治的是路径 ②。
2. **原生 `show()` 清 `pendingAttachNotify`**：治路径 ③。
3. **前端 `FloatingPetStatus.queried`**：把「查不到」和「确实不在悬浮窗」分开。
   轮询遇到 `queried === false` 直接跳过这一轮 —— **绝不把「没问到」当成「已收回」**。
   治路径 ①。
4. **前端连续确认**：`returnedStreak` 连续 `RETURNED_CONFIRM_ROUNDS`(2) 轮都是
   `detached === false` 才收回。收回是不可逆的，单轮中间态不作数。
5. **前端复核**：`confirmReturnedToApp()` —— 收到 `pet-attached` 不直接照做，而是
   回问原生一次；原生说 WebView 还在悬浮窗里，就判定为残响、丢弃并恢复形态。
   这是路径 ② 的**第二道防线**（事件一旦 `evaluateJavascript` 出去就撤不回）。
6. **`MainChat.healStuckFloatingState` 兜底自愈**：`MainChat` 挂载 **1.5 秒后**
   查一次原生，若发现 `detached === true` 而当前路由**不是** `/pet`，就补一次导航
   把桌宠页送回去。「页面不在 /pet 但 WebView 在悬浮窗」本身是非法状态，这条兜底
   保证即使前 5 条全漏，用户也不用手动重启。

   ⚠️ **必须延迟 + 二次确认，不能一读到就跳**：正常收回（点 ✕）时也会短暂出现
   完全一样的读数 —— `PetMode` 是「先 `handleReturnedToApp()` 切路由、再
   `hideFloatingPet()`」，而原生的 `hide` 命令内部还有 `HIDE_DELAY_MS`(80ms)
   延迟。这段窗口里 MainChat 已挂载、页面已是 `/chat`，而原生还没摘窗口；
   此时若立刻跳回 `/pet`，用户会看到「刚收回又被弹回桌宠页」。
   1.5s 远大于 80ms + IPC 往返，等一拍再查就安全了。

   另有一条对称的回滚：`handleReturnedToApp()` 里 `router.push('/chat')` 若**没落地**
   （页面仍停在 `/pet`），说明形态已切成桌面端而窗口还是悬浮窗那个小矩形 ——
   正是「角色凭空消失」的终态。那里会回滚为悬浮窗形态并让轮询继续跑。
   这是**唯一**能自愈它的地方：MainChat 没挂载，上面那条兜底跑不到。

> 同一处的两个连带修正：`syncFloatingPetState` 原来用裸 `invoke` 的 `isVisible()`
> 并把异常吞成 `false`，会让按钮误显示成「未启动」；用户再点一下就会走 `show()`
> 的**幂等分支**，把窗口整个重建（位置重置到 `(0,0)`、WebView 重新挂载 → 角色
> 闪没）。现在改成聚合查询 + `queried` 判断，查不到就保持原状态。
> `goToPetMode` 同理：先看 `status.detached`，已经在悬浮窗里就只补导航、不再 `show`。

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

- 页面**始终**按同一套逻辑画布排版（**210dp 宽**，见下），不做响应式
- `#pet-app` 用 `transform: scale(window.innerWidth / 210)` 缩放到窗口大小
- 窗口**宽度**由原生按屏幕比例算（只有原生知道真实屏宽）
- 窗口**高度**由页面按实际内容算好后通过 `set_size` 报回原生
  —— 气泡是流式输出的，高度随时在变，写死常量必然算错

> **⚠️ 逻辑宽度是 210，不是桌面端的 240。**
> 桌面端 240 = 头像圆框 210 + **两侧各 15 的「呼吸边」**，那 15px 是给
> 悬停按钮（设置/自动/返回/截图，`-left-3.5`）和光晕留的。悬浮窗里这排
> 按钮被 `v-if="!floatingMode"` 全部隐藏 → 那 15px 变成**纯透明空白**，
> 而头像在画布里是水平居中的，于是左右各露出一圈。详见 5.1.1。

| 状态     | 窗口宽度          | 逻辑高度           | 缩放系数（360dp 屏） | 显示内容      |
| -------- | ----------------- | ------------------ | -------------------- | ------------- |
| 收起     | 1/6 屏宽（60dp）  | 头像带 210         | 0.2857               | 仅头像        |
| 展开     | 0.6 屏宽（216dp） | 头像 210 + 输入 70 | 1.0286               | 头像 + 输入框 |
| 气泡出现 | 同上              | 再加气泡实际高度   | 同上                 | 窗口向下长高  |

宽度与高度**互相构造**：逻辑宽 210、`fit = 窗口宽 / 210`，所以
`逻辑宽 × fit ≡ 窗口宽` 恒等成立，**横向不可能出现空白**；高度由内容
决定，因此页面额外用 `max(内容高, 视口高 / fit)` 兜住「窗口比内容高」
那一半（见 `PetMode.vue` 的 `floatingCanvasHeight`）。

展开态取 0.6 屏宽而不是更窄：逻辑宽 210 时系数约 1.03（内容基本 1:1），
15px 的字渲染成约 15.4px；2/5 屏宽（144dp）时系数只有 0.69，字会小到看不清。
**这是这个方案的固有代价：文字绝对大小与窗口宽度成正比。**

`MIN_SIZE_DP` 必须够小（现为 24dp）：收起态窗口只有约 60×60dp，
原来的 80dp 下限会把前端报上来的高度硬抬到 80，下方凭空多一块透明区。

#### 5.1.1 那圈「呼吸边」—— 一个**已被推翻**的假设

> ⚠️ **本节结论是错的，保留只为记住排查路径。**
>
> - 本节主张「头像左右各 13.5px 透明边是唯一成因」。但 5.1.2 ② 的逐像素
>   测量显示：宠物**横向铺满了整张画布**，那 13.5px 在像素上量不出来
>   （Live2D 画布本身比头像容器宽，会溢出到容器外）。
> - 「缩小正常、放大不正常」的真正分水岭是 **`FLAG_NOT_FOCUSABLE`**，
>   而它导致的后果是 **5.1.4** 的「可触摸区域 = 整块屏幕」。
>
> 也就是说：这里量到的「14px」既不是可见的透明边，也不是用户看到的那片
> 区域的成因。当时把它当成真凶，方向整体偏了一轮。

下面的推导过程本身没错，错的是「它解释了什么」：

**① 诊断浮层的原始数字（窗口 216×252）**

```
canvas=216x252@0,0      ← 画布恰好铺满窗口，画布本身没问题
avatar=189x189@14,0     ← 189 = 210×0.9；x=14 ≈ (216-189)/2 = 13.5
```

画布与窗口都正确，**多出来的就是头像左右那圈**。

**② 截图的像素级测量（同一张图）**

| 量什么   | 方法                                                         | 结果               |
| -------- | ------------------------------------------------------------ | ------------------ |
| 窗口原点 | 诊断浮层是 `position:fixed;left:0;top:0`，其左上角即视口原点 | (9, 246) CSS px    |
| 画布矩形 | 品红底色（`rgba(255,0,255,.22)`）只在画布内，按行统计覆盖率  | 216×252 @ (8, 244) |

两者重合 → **画布确实铺满了窗口**，问题在画布内部。

**为什么「缩小正常、放大不正常」**：占比恒定（15/240 = 6.25%/侧），
但绝对宽度随窗口线性放大——

| 状态 | 窗口宽 | fit  | 单侧透明边           | 观感                 |
| ---- | ------ | ---- | -------------------- | -------------------- |
| 收起 | 60dp   | 0.25 | **3.75px**（≈0.8mm） | 看不见 →「正常」     |
| 展开 | 216dp  | 0.90 | **13.5px**（≈2.9mm） | 一眼可见 →「不正常」 |

**修法**：把悬浮窗的逻辑画布宽度从 `PET_WIDTH_BASE`(240) 换成
`FLOATING_LOGICAL_WIDTH`(= `AVATAR_BAND_BASE` = 210)，让头像铺满画布。
副作用是宠物放大约 14%（`fit` 0.9 → 1.03），文字更好读。

> 改这个常量必须**同时**改 Kotlin 侧的 `FLOATING_LOGICAL_WIDTH`：
> 窗口高度、`currentScale()`、`status.scale` 三处都由它换算
> （`COLLAPSED_HEIGHT_RATIO` / `EXPANDED_HEIGHT_RATIO`）。
> 两边不一致会直接表现为「窗口高度和画布对不上」。

#### 5.1.2 那圈东西里，有一半是**诊断代码自己画出来的**

把 240 改成 210 之后用户仍报「还有」。重新逐像素量了同一批截图，
发现两件之前被忽略的事：

**① 诊断浮层在画布上铺了品红底色、给每条带描了边 —— 那本身就是「一圈」**

```js
// 改之前（PetMode.vue 的 renderDiagnostic）
if (canvasEl) canvasEl.style.backgroundColor = "rgba(255,0,255,0.22)";
outlineOf(avatarContainer.value, "#22d3ee"); // 头像容器一圈青色
outlineOf(document.getElementById("pet-app"), "#ff00ff");
```

宠物是**非矩形**的，画布是矩形的 —— 画布底色在宠物轮廓之外露出来的那
一圈，加上头像容器那圈 1px 青色描边，肉眼看起来就是「宠物外面套了一圈」。
**诊断代码把被诊断的现象自己制造了出来**，而且它跟着每一版包一起发布
（`DEBUG_FLOATING_OVERLAY = true` 是硬编码的）。

现在整条描边/底色通道关掉了，只留左上角一行文字读数。

**② 逐像素测量证明「画布内并没有那圈呼吸边」**

对宠物所在行段（y 728..1200，画布 216×252）逐列统计品红覆盖率：

```
x=30..666（画布内 CSS 2..214）  覆盖率 0.00 ~ 0.01
```

也就是**宠物横向铺满了整张画布**，并不存在 13.5px 的左右透明边。
之前从 `avatar=189x189@14,0` 推出的「13.5px 呼吸边」，
在像素上量不出来 —— 因为 Live2D 画布本身比头像容器（210 逻辑）宽，
会溢出到容器外面去。**那 15px 从来就不是可见的透明边。**

⇒ 结论：诊断层自绘的品红底 + 青色框**确实**是「一圈」的可见来源之一，
必须删掉（已删）。但它解释不了用户真正的痛点 ——
「**整屏都能摸、摸着还能把桌宠拖走**」。那是 5.1.4 的
`FLAG_NOT_TOUCH_MODAL`，本节这一层只是同一症状里较小的那一半。

#### 5.1.3 缩放系数必须是**现算**的，不能是缓存

这是「画布比窗口小」这一类问题的结构性根治。

`--app-width` 是常量 210，`--pet-fit` 早先读的是 `floatingFit` 这个 ref，
而它由三条**异步**通道更新（原生推 `pet-metrics` / 500ms IPC 轮询 /
`resize` 事件），最多滞后 500ms。在滞后的那段时间里画布是按**旧**系数
渲染的：视口已经变成 216 宽，画布还按收起态的 0.25 渲染成 52 宽 ——
右边 164px 全是透明。

改成本 `liveFit`：直接从 `viewportSize` 现算，与 `--app-width` 在**同一次
渲染**里取值，于是

```
画布渲染宽 = --app-width × --pet-fit = 210 × (视口宽 / 210) ≡ 视口宽
```

是恒等式，不是估算 —— 画布**不可能**比 WebView 窄。

配套两件事：

| 改动                                                               | 为什么                                                                                                                                                      |
| ------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 新增 `#pet-shell`（`position:absolute; inset:0; overflow:hidden`） | `#app` 是 `fixed` + `100dvw/100dvh`，本层 `inset:0` 套在里面，尺寸**就是** WebView 视口，不经过任何计算。外层被钉死成窗口本身                               |
| `ResizeObserver` 观察 `document.documentElement`                   | `viewportSize` 是 `liveFit` 的唯一数据源；而 `window.resize` 在 Android WebView 里**不保证派发**（实测抓到过「视口已从 360 变 216，一次 resize 都没收到」） |

`floatingFit` 现在只作为「`viewportSize` 还没同步过」（`w === 0`）时的兜底。

**顺带修了诊断文字本身的坑**：`enterFloatingLayout` / `handleReturnedToApp`
都是「改完 ref 立刻调用」，Vue 还没 flush，于是浮层里 `floating=` / `exp=`
是新值而 `canvas=` / `applied=` 是旧 DOM —— 前后误导过两轮排查
（`[enter]` 那帧 `inner=802x360` 却 `canvas=240x480` 就是这个原因）。
现在 `showViewportDiagnostic` 包了一层 `nextTick`。

#### 5.1.4 **真正的成因：少写了一个 `FLAG_NOT_TOUCH_MODAL`**

前面 4.4 / 5.1.1 / 5.1.2 三节分别在讲「矩形窗口的固有代价」「呼吸边」
「诊断层自绘」，它们各自都是真问题，但**都不是用户真正在报的那件事**。

用户的原话是：

> 外面依旧很大一片透明区域**可以被触摸来拖动桌宠**
> 我忘记强调一个点，**区域是覆盖了整个屏幕**
> 悬浮窗展开外面**一整屏**的区域是完全透明的
> 反正我**在其他应用**时也会这样，**和主活动应该没关系**

把这三句拼起来，指向的其实是一件很具体的事：**悬浮窗的可触摸区域不是它
自己那块矩形，而是整块屏幕。** 能证明这一点的正是「在别的应用里也这样」
—— 那里根本没有我们的 Activity，唯一的嫌疑对象就是悬浮窗自己。

Android 官方对 `FLAG_NOT_TOUCH_MODAL` 的定义（`WindowManager.LayoutParams`）：

```
Window flag: even when this window is focusable (its FLAG_NOT_FOCUSABLE
is not set), allow any pointer events outside of the window to be sent
to the windows behind it. **Otherwise it will consume all pointer events
itself, regardless of whether they are inside of the window.**
```

最后半句就是全部答案：**不给这个 flag 的窗口，触摸区域是整块屏幕。**

于是窗口外的每一次触摸也被投递给它 → `buildPetTouchListener()` 的
`ACTION_DOWN` 触发 → 窗口跟着手指走。用户看到的就是「外面很大一片区域
可以被触摸来拖动桌宠」，**而且在别的应用里一模一样**。

**为什么偏偏展开态才明显**

| 状态 | `FLAG_NOT_FOCUSABLE`                        | 系统行为                                                                                  |
| ---- | ------------------------------------------- | ----------------------------------------------------------------------------------------- |
| 收起 | **有**                                      | 不可获焦窗口，系统处理收敛，触摸基本只落在窗口内 →「缩小的时候正常」                      |
| 展开 | **被 `setExpanded` 摘掉**（为了能弹输入法） | 窗口可获焦，上面那句「consume all pointer events」完整生效 → 整屏吃触摸 →「放大就不正常」 |

所以 5.1.1 里那个「`FLAG_NOT_FOCUSABLE` 是分水岭」的直觉是**对的**，
但推下去的结论错了：它引出的不是「呼吸边」，而是「整屏触摸」。

**修法（一行）**

```kotlin
WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE or
    WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL or   // ← 本次新增
    WindowManager.LayoutParams.FLAG_LAYOUT_NO_LIMITS or
    WindowManager.LayoutParams.FLAG_HARDWARE_ACCELERATED,
```

常驻即可，收起态留着也无害。`setExpanded` 只做
`flags or / and inv(FLAG_NOT_FOCUSABLE)`，不重建 flags，所以两个形态下都
不会被弄丢 —— 这一点很关键，**任何把 `params.flags` 整体赋值的写法都会
把这个 flag 抹掉**。

**这次真正的固有代价有多大**

只剩「**窗口矩形以内、宠物轮廓以外**」那几个角落：
收起态 60×60dp、展开态 216×252dp 之内的透明角落。
**窗口之外的一切触摸都会正常落到下层应用上。**

> 📌 **排查教训**：当一个症状是「触摸行为不对」时，先问「**触摸区域**是谁
> 定的」，而不是去量「可见区域」有多大。前两轮全部时间都花在量像素、
> 量画布、量呼吸边上，而真正错的是一行 flag。另外，用户说「在别的应用里
> 也这样」是一句**极强的定位信息**——它一次性排除了宿主 Activity 这个
> 嫌疑对象，应该第一时间被当作主线而不是补充说明。

#### 5.1.5 折叠时会「闪一下」 —— 视口跟不上窗口

真机现象：折叠（展开态 → 头像态）时界面闪一下。

**成因**：折叠要同时改两样东西 —— 窗口尺寸（原生）和缩放系数（页面），而它们
**不可能在同一帧里生效**：

```
t=0    原生 updateViewLayout：窗口 216dp → 60dp
t=0    页面还在按展开态的系数（约 1.03）渲染
       → 画布渲染宽 = 210 × 1.03 ≈ 216 CSS px，而窗口只有 60dp
       → 内容被裁掉一大块
t+几帧 WebView 视口才跟上 → liveFit 重算 → 内容缩回来
```

中间那几帧就是「闪」。

**为什么「提前推系数」单独不够**：`--pet-fit` 是 `liveFit` **从视口现算**的
（见 5.1.3），原生推的 `floatingFit` 只在视口尺寸为 0 时兜底。所以推得再早，
页面用的还是「从**旧**视口算出来的旧系数」。

**修法**（两侧各一处，缺一不可）：

1. **原生**：把 `notifyMetrics(params, view)` 挪到 `updateViewLayout` **之前**。
   它读的是 `params.width`（此时已是新值），所以推出去的就是新系数 ——
   页面能在窗口变化**之前**拿到它。
2. **前端**：`liveFit` 在「刚收到原生即时推来的几何」且「与现算值对不上」时，
   改用原生的权威系数：

   ```ts
   const auth = floatingFit.value;
   if (
     auth > 0 &&
     fromViewport > 0 &&
     Date.now() - authoritativeAt < AUTHORITATIVE_TTL_MS && // 刚推过
     Math.abs(fromViewport - auth) / auth > 0.02 // 确实对不上
   ) {
     return auth;
   }
   ```

   `authoritativeAt` **只**在 `onPetMetrics` 里打戳 —— 500ms 轮询也会更新
   `floatingFit`，但那是滞后值，不能当权威值用。

两个条件缺一不可：只判「对不上」会在稳定态被浮点抖动误触发；只判「刚推过」
会在推送内容与视口本就一致时白白绕开现算。

视口一跟上两者就相等，自动切回现算 —— 而现算正是「画布宽 ≡ 视口宽」那条
恒等关系的来源（见 5.1.3），不能丢。

> 这条同时解释了「展开后一大片空白」：那是**同一个滞后**的另一个方向
> （画布比窗口小 → 露透明）。所以 5.1.3 的「现算」和这里的「权威值顶替」
> 不是互相矛盾，而是分工：**稳定态用现算保恒等，过渡态用权威值保同步。**

### 5.2 手势：拖动 / 点头像 / 左侧按钮收回

手机没有鼠标，桌面端那套 `mouseenter/mouseleave` 完全不适用：

| 手势                     | 行为                                 |
| ------------------------ | ------------------------------------ |
| 拖动                     | 移动窗口，松手后吸附到最近的左右边缘 |
| 单击头像                 | 展开 / 收起**来回切换**              |
| 展开后点左侧「返回主页」 | 收回悬浮窗，切回 App 并跳回聊天页    |

**为什么取消了「双击收回」**：双击和「点头像展开/收起」是**直接冲突**的
——同一位置的两次点按，既可能是「展开 → 收起」，也可能是「收回」，
物理上无法区分。而且原先的判定只看时间不看位置，任意两次 300ms 内的点按
都算双击（点完头像紧接着点输入框也会把桌宠收回去），真机误触严重。

现在收回由左侧那排圆形按钮里的 **「返回主页」** 负责（见 5.3），手势只剩
「单击头像 = 展开/收起」一种，没有歧义。

**收回键不再单独造**：早先版本在展开面板右上角单放了一个 ✕ 按钮，后来删掉了。
桌面端本来就有一排左侧圆形按钮（设置 / 自动 / 返回主页 / 截图 / 语音），
「返回主页」就在里面 —— 同一件事有两套入口，图标和位置迟早对不上。
悬浮窗里需要做的只是让那一排**别被 `v-if="!floatingMode"` 隐藏、别被
`#pet-app` 的 `overflow-hidden` 裁掉**，而不是另造一个键。它和电脑端一样是
「悬停才浮现」的，只是悬浮窗里的「悬停」由展开态提供 —— 所以收回悬浮窗
要**先点头像展开**，那一排才出现。详见 5.3。

拖动与点击的区分靠位移阈值（`TAP_SLOP_DP = 16dp`）：没有它，每次拖完都会误触发点击。
取 16dp 而不是 Android 默认的 8dp——这里判定的是「整个窗口要不要跟着手指走」，
不是滚动，手指点按时天然会带几 dp 位移，8dp 会把大量正常点按判成拖动
（表现为「单击经常没反应，窗口还会被带偏一点」）。
一旦判定为拖动，会向 WebView 补发一个 `ACTION_CANCEL`，否则页面那边会一直
停在「按下未抬起」的状态（按钮保持按压态）。

### 5.3 悬浮窗里的左侧按钮排：只有**定位**要换，显隐时机与电脑端一致

桌面端那排圆形按钮（设置 / 自动 / 返回主页 / 截图 / 语音）挂在头像框**左外侧**
（`-left-3.5` = -14px），平时透明、悬停（`.is-hovered`）才浮现。

悬浮窗里唯一不成立的是**定位**：

| 桌面端的做法                        | 悬浮窗里为什么不行                                                                                                                                                                                                        | 悬浮窗里的做法          |
| ----------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------- |
| 挂头像框左外侧 `-left-3.5`（-14px） | 那 14px 落在 `PET_WIDTH_BASE`(240) 比 `AVATAR_BAND_BASE`(210) 多出来的「呼吸边」里，而悬浮窗的**逻辑画布宽度就等于头像带宽**（见 5.1）——负偏移会落到画布外，被 `#pet-app` 的 `overflow-hidden` 整个裁掉，真机上根本点不到 | 贴画布**内侧** `left-1` |

**显隐时机两个形态逐字共用**，因为悬浮窗把「悬停」换成了「展开态」：

| 事件               | 电脑端         | 悬浮窗                    |
| ------------------ | -------------- | ------------------------- |
| 进入 `.is-hovered` | 光标移入头像框 | **展开**（`petExpanded`） |
| 离开 `.is-hovered` | 光标移出头像框 | **收起**                  |

也就是：电脑上光标一离开，这排按钮就收起；悬浮窗里一收起，这排按钮就消失 ——
时机一一对应，连 300ms 的过渡都是同一份 CSS。手机上没有「悬停」，展开态就是它
在这台设备上唯一合理的对应物。

实现落在 `GameRolesStage.vue` 的两个 computed 上：

```ts
// 1) 悬停态：桌面端用指针判定，悬浮窗直接取展开态
const isStageHovered = computed(() =>
  floatingMode.value ? props.expanded === true : pointerHovered.value,
);

// 2) 定位与显隐：显隐那半段两个分支**逐字相同**，只有定位不同
const sideButtonClass = computed(() =>
  floatingMode.value
    ? "left-1 pointer-events-none translate-y-2 opacity-0 group-[.is-hovered]:pointer-events-auto group-[.is-hovered]:translate-y-0 group-[.is-hovered]:opacity-100"
    : "-left-3.5 translate-y-2 opacity-0 group-[.is-hovered]:translate-y-0 group-[.is-hovered]:opacity-100",
);
```

共享的静态类（圆底、描边、`backdrop-blur`、hover 放大）留在各按钮自己的 `class`
里，只有「两个形态不一样」的部分进这个 computed。

角色铭牌（头像右上角的名字条）走的是**同一个** `.is-hovered`，因此一起跟着展开态
显隐，不需要另一套规则。它比按钮多一处调整：`-right-4` 在悬浮窗里同样会越出 210
宽的画布，改成 `right-1`（`GameRoleAvatar.vue` 的 `floatingWindow` prop）。

#### 5.3.1 悬浮窗为什么必须补一条 `pointer-events-none`

悬浮窗的画布只有 210 宽，这排按钮**压在头像上**。`opacity-0` 的元素照样接收触摸，
于是收起态点头像想展开、却会先命中那个看不见的按钮（比如直接打开设置）。

电脑端不存在这个问题（按钮在头像外侧的呼吸边里），所以这条只加在悬浮窗分支。
展开后 `.is-hovered` 生效，`group-[.is-hovered]:pointer-events-auto` 再把触摸放开。

#### 5.3.2 悬浮窗里**不能**沿用指针判定悬停

Android WebView 会把触摸合成成 pointer 事件（`pointerdown` / `pointermove`），
手指落在窗口边角（宠物轮廓之外、`#pet-app` 之内）就会派发一次。若沿用它判定悬停，
等于把按钮和铭牌的显隐交给「有没有摸到窗口角落」这种随机事件 —— 真机表现就是
展开后摸一下，它们莫名其妙地闪一下、或者卡住不消失。

因此 `syncStageHover()` 在悬浮窗里**提前返回**（不写 `pointerHovered` —— 那个值在
悬浮窗里没有任何接收者，写进去只会白白触发一次响应式更新）。

`top-*` 两个形态共用：`top-1 / top-10 / top-19 / top-28 / top-37` 对应逻辑
y = 4 / 40 / 76 / 112 / 148，最下面那个按钮底边 148+32 = 180，仍在 210 高的头像带内
—— 收起态（窗口约 60dp）也不会越出窗口。

> ⚠️ **`v-if="!floatingMode"` 必须删干净，但 `:class` 只能有一个。**
> 「自动」按钮同时需要定位类和「自动模式开启」的高亮类，必须并进**同一个数组**
> —— 写成两个 `:class` 的话，后写的那个会整体覆盖前一个，定位就丢了。

## 六、后续阶段

| 阶段      | 目标            | 关键工作                                          |
| --------- | --------------- | ------------------------------------------------- |
| **P0** ✅ | 技术可行性验证  | 插件骨架、CI 编译通过                             |
| **P1** ✅ | 搬运主 WebView  | 占位页、view 搬运、TCP 保留（IPC/store 完整可用） |
| **P2** ✅ | 手机端交互      | 屏幕比例尺寸、拖动/点头像/左侧按钮收回、展开收起  |
| **P3** ✅ | 逻辑画布 + 缩放 | 整体等比缩放、内容高度回报、WebView 保活轮询      |
| **P4**    | 气泡与打磨      | 气泡位置策略、权限引导 UI、厂商白名单             |

> 原 P3「气泡悬浮到窗口外」已作废：`FLAG_LAYOUT_NO_LIMITS` 是让**窗口本身**
> 可以超出屏幕边界，**不是**让 WebView 内容画出自己的 bounds。WebView 内容
> 永远被裁剪在自身范围内，气泡不可能靠它跑到窗口外。现在气泡就在窗口内，
> 由窗口跟着长高来容纳（见 5.1）。

## 七、发布前必须处理的风险

| 风险         | 说明                                                                        |
| ------------ | --------------------------------------------------------------------------- |
| **上架政策** | Google Play 对 `SYSTEM_ALERT_WINDOW` 审核严格，需陈述必要性；国内商店较宽松 |
| **厂商限制** | 小米/华为/OPPO 等需额外加入「后台弹出界面」白名单，否则悬浮窗被拦截         |
| **后台存活** | 需 `FOREGROUND_SERVICE` + 常驻通知，否则切后台后悬浮窗可能被回收            |
| **权限引导** | 该权限无法运行时弹窗申请，只能跳设置页；用户返回后需重新检测                |

> 注：本项目为 **AGPL-3.0**，代码本就要求开源，无闭源商业化顾虑。

## 八、怎么测试

### 8.1 拿安装包

**本地出包是现在的主路径**（本机已配好完整 Android 工具链，见 9.1）：

```bash
cd /d/LingChat-BuildEnv
bash build-android-pet.sh      # 约 10–13 分钟，产出未签名 APK
bash sign-apk-pet.sh           # zipalign + apksigner → *-signed.apk
bash verify-pet-apk.sh         # 验包：确认改动真的进了包（见下）
```

`verify-pet-apk.sh` 这一步别省：Android 包里前端资产是 brotli 压缩、Kotlin 是 R8 混淆过的，
**直接 `grep` 字符串搜不到**，很容易出现「改了、打了包、装上去没变」而查不出原因的情况。

CI 侧仍然可用（不上架、不建 Release），但**不是默认路径**：

```bash
gh workflow run dev-build-android.yml --repo <你的fork> --ref feat/android-floating-pet
gh run download <run-id> --repo <你的fork> -n lingchat-dev-android
```

### 8.2 真机验证步骤

1. 安装 APK，启动 App，进入聊天主界面
2. 点右上角**「桌宠」**按钮
3. **首次**会提示需要悬浮窗权限 → 跳系统「显示在其他应用上层」设置页
   → 打开 LingChat 开关 → 返回 App
4. **再点一次「桌宠」** → 短暂切到 `/pet` 页后，WebView 被搬进悬浮窗，
   桌面上出现**仅头像**的小窗（约 1/6 屏宽）
5. 此时切回 App：应看到**占位引导页**（而不是白屏），说明 WebView 已搬走。
   占位页文案必须**清晰可读**（不透明近黑底 + 白字），不该直接压在壁纸上
6. **单击头像** → 窗口变大到约 2/5 屏宽，出现输入框
7. **看左侧那排圆形按钮** → **收起态应当看不见**（与电脑端「光标不在头像上」一致），
   点一下头像展开后，设置 / 自动 / 返回主页 / 截图 / 语音应全部浮现、贴画布内侧、
   不越出窗口边界；同时右上角出现角色铭牌
8. **收起态点头像** → 应该正常**展开**，而不是打开设置
   （按钮隐藏时若不挡触摸，最先命中的就是它，见 5.3.1）
9. **在输入框里发一条消息** → 应能正常发送并收到回复
   （这是搬运方案的核心验证点：IPC 与 store 都还在）
10. **拖动头像** → 窗口跟随移动，松手后吸附到屏幕边缘
11. **点左侧「返回主页」** → 收回悬浮窗，回到 App 主界面（并自动跳到聊天页）
12. **横屏拖拽**：把手机转成横屏，拖动桌宠 → 必须能拖到屏幕**右半边**，
    不能被「竖屏宽度」那道看不见的墙挡住
13. **来回旋转**：竖屏 ↔ 横屏至少切 3 次，分别从「贴左沿」和「贴右沿」两种
    位置开始 → 桌宠都不能跑出屏幕，也不该莫名跳到对面
14. **反复进出悬浮窗**（见 4.4.2.4）：点左侧「返回主页」收回，**立刻**再点一次
    「桌宠」重新进入 —— 至少来回 5 次，且刻意在收回后 3 秒内就重进
    （那正是 `pet-attached` 重试序列的跨度）→
    **悬浮窗里必须一直是桌宠页**，不能变成聊天页，角色也不能消失
15. **折叠不闪**（★ 本轮重点，见 5.1.5）：在**横屏**和竖屏各展开 → 折叠 5 次
    → 折叠过程中内容应该平滑缩小，**不该出现「被裁掉一块再缩回来」的闪**
16. **横屏边界**（★ 本轮重点，见 4.4.2.1）：转成横屏后，桌宠要能拖到屏幕
    **右半边**，不能被「竖屏宽度」那道看不见的墙挡住；横屏下展开/折叠也要正常
17. **确认没有诊断浮层**：屏幕左上角不该再出现绿字的 `[enter] / [live]` 读数框

> 转屏相关的问题排查时，先看 logcat 里 `FloatingPet` 的这行：
> `屏幕尺寸变化：悬浮窗重排为 … 屏幕 WxH（rotation=…, mode=…x…）`。
> 没有这行 = 旋转压根没触发重排；有这行但尺寸是竖屏的 = 读数取错了源。

### 8.3 当前能验证到哪一步

| 能力                                       | 状态                                                                    |
| ------------------------------------------ | ----------------------------------------------------------------------- |
| 搬运主 WebView 进悬浮窗                    | ✅                                                                      |
| 浮在其他 App 之上                          | ✅                                                                      |
| Activity 占位页（防白屏）                  | ✅                                                                      |
| **IPC / store 完整可用**                   | ✅ 这是搬运方案相对「新建 WebView」的关键收益                           |
| 角色/台词显示                              | ✅ 数据来自原有 store，无需镜像                                         |
| 在悬浮窗里发消息                           | ✅ 复用原有 `ChatInput` 与 Tauri 命令                                   |
| 拖动移动 + 边缘吸附                        | ✅ 边界读数走 `DisplayManager`，横屏能拖到右半边（见 4.4.2.1）          |
| 点头像展开/收起                            | ✅                                                                      |
| 左侧按钮排（设置/自动/返回主页/截图/语音） | ✅ 贴画布内侧；显隐时机与电脑端一致（展开才浮现，见 5.3）               |
| 角色铭牌                                   | ✅ 与按钮同一套时机（展开才浮现，见 5.3）                               |
| 点左侧「返回主页」收回并跳回聊天页         | ✅ `pet-attached` 事件 + 轮询双路；误判有三层防护（见 4.4.2.4）         |
| 反复进出悬浮窗不被误判成「已收回」         | ✅ 原生 `petModeEpoch` 作废跨轮事件 + 前端复核 + 导航自愈（见 4.4.2.4） |
| 横竖屏旋转后仍在屏幕内                     | ✅ `DisplayManager` 回调 + 500ms 轮询双保险，两个方向都覆盖（见 4.4.2） |
| 退后台后继续运行                           | ✅ 前台服务保进程 + 500ms 轮询保 WebView                                |
| 气泡在窗口内随内容长高                     | ✅ 窗口高度由页面回报给原生                                             |

### 8.4 权限被拒 / 找不到开关

- **国产 ROM**：小米/华为/OPPO 等除「显示在其他应用上层」，还需在
  「后台弹出界面」「自启动」里放行，否则悬浮窗被静默拦截
- **没弹设置页**：部分系统该权限默认关闭且无入口，属系统限制
- **排查**：看 logcat 中 `FloatingPet` 标签的输出

### 8.5 已知风险点（真机重点观察）

> ⚠️ **下表里凡是让你「看诊断浮层」的行，现在都不再适用。**
> 那段临时诊断（`DEBUG_FLOATING_OVERLAY`、`renderDiagnostic`、
> `showViewportDiagnostic`、画布品红底色与各带描边）**已经整段删除** ——
> 它自己会画出用户可见的形状，把被诊断的现象制造出来（见 5.1.2）。
> 那些行保留下来只作为**成因解释**；下次要量几何，得先把诊断重新打开。

| 现象                                                     | 可能原因                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| -------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 切回 App 白屏                                            | 占位页没生效，`setContentView` 顺序有问题                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| 收回后主界面黑屏                                         | WebView 搬回失败，需看 `restoreWebViewToActivity` 日志                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| **收回后只剩左上一角、不回聊天页**                       | `pet-attached` 没送达。三层保险：`notifyWeb` 显式收 WebView 参数、0/300/1000/3000ms 重试、Activity resume 时由 `ensureLifecycleCallbacks` 补发                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| **收回后整个界面缩在左上角**                             | **`setContentView` 没重置 LayoutParams**（见 4.4）。先量 `window.innerWidth` 和屏宽对照：等于悬浮窗宽度就是这条，等于屏宽则是视口问题                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| **点 ✕ 收回直接闪退**                                    | **`setContentView` 之后事后赋值 `view.layoutParams` → `FrameLayout.onMeasure` 的 ClassCastException**（见 4.4）。改成两参重载即可。已从「startActivity flag」「moveTaskToFront」等嫌疑点排除                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| **收回后整个 App 用窄视口渲染**                          | 先量 `window.innerWidth`。等于屏宽就是视口问题；等于悬浮窗宽度则是 LayoutParams 没改回 MATCH_PARENT                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| **展开后一大片空白（内容挤在左上角）**                   | 两种成因，先看诊断 `gap L…T…R…B…` 那一行定位：**左右有余量** = 缩放系数错（`notifyMetrics` 曾用 `view.width`，它在 `updateViewLayout()` 之后是上一形态的值）；**上边为负 / 下边大** = 内容被系统顶偏了（见下一条）                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| **展开后内容被顶到屏幕上方、下方空一片**                 | 展开态摘掉 `FLAG_NOT_FOCUSABLE` 后，系统开始插手窗口内容区（insets / 输入法），而画布尺寸只由窗口**宽度**推出，高度被改它不知情（见 4.4）。检查 `SOFT_INPUT_STATE_VISIBLE` 是否还在、`fitInsetsTypes` 是否设了 0；诊断里看 `scroll=` 与 `doc=`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| **展开后透明区特别大、且随桌宠缩放变**                   | 页面走了桌面端分支（`PET_WIDTH_BASE × pet.scale`，没有 `--pet-fit`）。看诊断 `applied=` 与 `role scaleP=`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| **展开后四周一圈透明**                                   | ✅ **已修**（见 5.1.1 / 5.1.2 / 5.1.3）。**三个来源，别再只查一个**：① **诊断代码自绘** —— 画布上的品红底色 + 头像容器的青色描边，宠物是非矩形，露出来的那一圈看起来就是「套了一圈」（见 5.1.2，已全部关掉）；② 画布**外**的透明 —— 系统给窗口加了 insets，WebView 比窗口小一圈（见 4.4）；③ 缩放系数用了**缓存值** —— `--pet-fit` 滞后 500ms 时画布按旧系数渲染，比窗口小（见 5.1.3，已改成现算的 `liveFit`）。诊断里看 `gap` 行（全 0 才对）与 `shell` 行（必须等于 `inner`）                                                                                                                                                                                                                                                 |
| **展开后「一整屏」都能被触摸、摸着还能把桌宠拖走**       | **少写 `FLAG_NOT_TOUCH_MODAL`**（见 5.1.4）。没有它，窗口的可触摸区域是**整块屏幕**，窗口外的触摸也被投递给它 → `buildPetTouchListener` 的 ACTION_DOWN 触发 → 窗口跟手走。**在别的应用里同样发生**（那里没有我们的 Activity，正好可用来确认）。修法是一行 flag，且注意别用整体赋值 `params.flags = ...` 把它抹掉                                                                                                                                                                                                                                                                                                                                                                                                                |
| 展开后**窗口矩形之内**、宠物轮廓之外的角落吃触摸（固有） | 悬浮窗是矩形、宠物是圆形，那圈角落**无法**逐像素穿透（见 4.4）。范围仅限窗口自身矩形（收起 60×60dp / 展开 216×252dp），**窗口之外不受影响**；看诊断 `band=`（不为 0 = 气泡带占了高没渲染）、`role scaleP=`（< 1 = 宠物只占头像框一部分）                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| **横屏下宠物被挤出屏幕**                                 | 尺寸基准用了 `screenWidthDp()`。横屏 0.6×屏宽 推出的高度超过屏高。已改为 `min(屏宽, 屏高)`（见 4.4）                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| **展开后右边空 120 / 下边空 322 的空白**                 | **`fit` 的三个来源全失效**（`pet-metrics` 推送、500ms IPC 轮询、`resize` 事件），系数停在初值 1.0。判据：诊断里 `win=0x0dp` 且标签停在 `[enter]`（没变成 `[live]`）。已加 500ms 本地心跳自愈（见 4.4.1），并把 `resize` 改成在 `onMounted` 里**无条件**绑定（早先只在「挂载时已是悬浮窗」的分支绑，而启动顺序是先 `push('/pet')` 再 `showFloatingPet()` → 那条分支永远走不到）                                                                                                                                                                                                                                                                                                                                                  |
| **画布高度比窗口矮，底部留一条透明**                     | 已加兜底：`floatingCanvasHeight = max(内容高, 视口高 / fit)`。宽度是构造出来的（`逻辑宽 × fit ≡ 窗口宽`），高度不是，必须显式兜                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| **点空白区域会收起输入框**                               | 空白落在 `#pet-app` 之外、窗口之内 → 手指派发 `mouseleave` → 桌面端的「光标离开即收起」。它是「空白存在」的旁证。已把悬浮窗里的 `mouseenter/leave` 改成空操作（见 4.4.1）                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| **横屏时桌宠只能停在左半边（右边一大片拖不过去）**       | 边界读数**经过 Activity 的 Resources**。这条走过**三版**、都被证伪：① `resources.displayMetrics`；② `maximumWindowMetrics`（AOSP 里它读的就是 `getResources().getConfiguration().windowConfiguration.maxBounds`，同一个 Resources）；③ `DisplayManager.getDisplay().getRealMetrics()` —— 看着离开了 Resources，实则内部 `adjustMetrics` 又按 `mResources` 的旋转交换一次，**还是绕回去**。桌宠悬浮时 App 已 `moveTaskToBack` 到后台，后台 Activity 的 Resources 不跟随旋转刷新 → 边界永远停在竖屏宽度。改用 `Display.getRotation()` + `Display.getMode()`（两者直读 DisplayInfo，见 4.4.2.1）                                                                                                                                   |
| **折叠（展开 → 头像）时界面闪一下**                      | 窗口尺寸与缩放系数**不可能同帧生效**：窗口先缩到约 60dp，页面还按展开态的大系数（约 1.03）渲染 → 画布比窗口大、内容被裁掉一块，几帧后才缩回来（见 5.1.5）。修法两侧各一处：原生把 `notifyMetrics` 挪到 `updateViewLayout` **之前**；前端 `liveFit` 在「刚收到原生即时几何」且「与现算对不上」时改用权威系数。⚠️ 这两条都是**顺序 / 取值**改动，验包脚本无法用字符串断言，只能真机确认                                                                                                                                                                                                                                                                                                                                           |
| **旋转屏幕后桌宠跑到屏幕外**                             | 三个成因，都在 4.4.2：① `x/y` 是屏幕坐标系里的**绝对值**，旋转后屏宽高对调就飞出屏幕；② **读数来源取错**，`lastScreenW/H` 永远不变 → 重排判据永不成立（见 4.4.2.1）；③ **回调根本不会来** —— `ComponentCallbacks.onConfigurationChanged` 只对可见 Activity 送达，而悬浮时 Activity 在后台。修法是 `DisplayManager` 的显示器回调 + 500ms 轮询兜底（见 4.4.2.2 / 4.4.2.3），**两个方向（横→竖、竖→横）都要验**                                                                                                                                                                                                                                                                                                                    |
| **竖屏↔横屏来回切，桌宠跑出屏幕 / 贴不到边**             | 同上。验证时至少来回切 3 次，并分别试「贴着左沿」和「贴着右沿」两种起始位置                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| **反复进出悬浮窗后，悬浮窗里变成聊天页 / 角色凭空消失**  | `handleReturnedToApp()` 被**误触发**（见 4.4.2.4）。**三条误判路径都要查**：① `getFloatingPetStatus()` 的 catch 把 IPC 失败伪装成 `detached: false` → 轮询当成「已收回」（现在靠 `queried` 字段拦住）；② 收回时排队的 `pet-attached` 重试（0/300/1000/**3000**ms）在 3 秒内重进悬浮窗后**迟到送达** → 命中新一轮监听器（**主因**，现在靠原生 `petModeEpoch` 作废 + 前端 `confirmReturnedToApp` 复核）；③ `show()` 没清 `pendingAttachNotify`，跨轮残留的引用会在 resume 时补发。注意这个 bug **不可逆**（`handleReturnedToApp` 会停掉轮询），所以还有第 6 条兜底：`MainChat.healStuckFloatingState` 在挂载 1.5s 后发现「页面不在 /pet 但 WebView 在悬浮窗」会补一次导航自愈（**必须延迟** —— 正常收回时也会短暂出现同样的读数） |
| **窗口尺寸变了但页面不跟**                               | `resize` 监听没绑上（`enterFloatingLayout` 里漏绑，见 4.4）。诊断里 `inner=` 与 `win=` 长期不一致就是这条                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| **退后台后桌宠不动**                                     | 保活轮询没起来；看 logcat 里 `FloatingPet` 的「已启动保活轮询」                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| **悬浮窗里点不到按钮 / 按钮看不见**                      | 左侧那排按钮仍在挂桌面端的 `-left-3.5`（落在画布外的「呼吸边」里，被 `overflow-hidden` 裁掉）。定位必须换、显隐时机**不该**换：它和电脑端一样是「悬停才浮现」，悬浮窗里的「悬停」由**展开态**提供（`isStageHovered`），所以收起态看不见是**正确行为**（见 5.3）                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| **收起态点头像，却打开了设置（或按钮闪一下）**           | 两个成因：① 隐藏的按钮仍在接收触摸 —— 悬浮窗里按钮压在头像上，需要 `pointer-events-none`（见 5.3.1）；② 悬停仍由指针事件判定 —— Android 会把触摸合成成 pointer 事件，落在窗口边角就触发一次（见 5.3.2）                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| **旋转后要等半秒才归位**                                 | `displayListener` 没注册上（`ensureDisplayListener`），只剩 500ms 轮询兜底。看 logcat 有没有「屏幕尺寸变化：悬浮窗重排为 …」                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| **气泡看不到 / 以为消息没发出**                          | 窗口高度没跟着内容长，气泡被裁；查 `reportFloatingHeight` 的 IPC 是否成功                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| **单击经常没反应、窗口被带偏**                           | `TAP_SLOP_DP` 偏小，正常点按被判成拖动                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| 悬浮窗里输入框弹不出键盘                                 | 窗口 `FLAG_NOT_FOCUSABLE` 没在展开态摘掉                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| 按住 Home 后悬浮窗消失                                   | 前台服务被 ROM 拦截，需加「后台弹出界面」白名单                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |

## 九、工程验证方式

### 9.1 本地构建（现在的主路径）

本机已配好完整 Android 工具链，**不必再走 GitHub Actions**：

```
D:\LingChat-BuildEnv\
├─ LingChat-Pet\            ← fork 的 feat/android-floating-pet 工作副本
├─ jdk\  android-sdk\  rust\  mingw\  gradle-home\   ← 工具链与缓存
├─ android-keystore\debug.keystore                    ← 调试签名
├─ build-android-pet.sh     ← 一键出包（对照 CI 步骤，pnpm 换成直接 node）
├─ sign-apk-pet.sh          ← zipalign + apksigner
└─ verify-pet-apk.sh        ← 验包：确认改动真的进了包（R8 混淆 + brotli 压缩后搜字符串的坑）
```

```bash
cd /d/LingChat-BuildEnv
bash build-android-pet.sh          # 约 10–13 分钟
bash sign-apk-pet.sh               # 产出 *-signed.apk，可直接覆盖安装
bash verify-pet-apk.sh             # 可选但强烈建议：确认改动真的进了包
```

构建前建议先跑前端类型检查（`build-android-pet.sh` 里刻意跳过 vue-tsc）：

```bash
cd /d/LingChat-BuildEnv/LingChat-Pet
node node_modules/vue-tsc/bin/vue-tsc.js --noEmit --skipLibCheck
```

### 9.2 一次构建里 Rust 会被编译两遍 —— 两个成因，只修掉了一个

`tauri android build` 会在**同一次**构建里起两趟 cargo：

|     | 触发者                                                         | 干什么                                          |
| --- | -------------------------------------------------------------- | ----------------------------------------------- |
| ①   | tauri CLI 自己（`mobile/android/build.rs::run`）               | 编 `libling_chat_lib.so`，再 symlink 进 jniLibs |
| ②   | Gradle 的 `BuildTask` 跑 `tauri android android-studio-script` | 把同一个 `.so` 再编一遍                         |

日志里就是两次 `Compiling ling_chat` + 两次 ``Finished `release` profile``（各约 6.5 分钟）。

**第二趟为什么不吃第一趟的缓存**：两趟注入给 cargo 的**环境变量不一样**。

- ① 在 `build.rs::run` 开头调了 `delete_codegen_vars()`，它把进程环境里 cargo 相关变量清掉，
  `CARGO_PROFILE_RELEASE_*` 一并消失 → 这趟 cargo 用 `.cargo/config.toml` 的
  `[profile.release]`，即 `opt-level = "z"`。
- ② `android_studio_script.rs::command` **没有**调 `delete_codegen_vars()`，而 tauri-cli 的
  `env_vars()` 白名单**包含 `CARGO_` 前缀**，于是 `CARGO_PROFILE_RELEASE_OPT_LEVEL=s`
  被原样转发给 cargo → 这趟用 `opt-level = "s"`。

profile 不一致 → cargo 判定 `ProfileConfigurationChanged` → 重编。
（CI 的 `build-android.yml` 同样设了这两个变量，所以 CI 也在白烧这 6.5 分钟。）

**为什么只重编 `ling_chat` 而不重编几百个依赖**：Cargo 的 `[profile]` 配置
**只作用于工作区成员**，所以受影响的恰好是 `ling_chat` 和 `tauri-plugin-floating-pet`
这两个本地 crate。

**定位手法（很值得复用）**：

```bash
CARGO_LOG="cargo::core::compiler::fingerprint=trace" cargo build ...
```

它会逐字打出 `dirty: <原因>`，比反复试错快得多。实测：

- 设了 `CARGO_PROFILE_RELEASE_OPT_LEVEL` → `ProfileConfigurationChanged` 出现 **0** 次
- 不设 → 恰好 **2** 次（`ling_chat`、`tauri-plugin-floating-pet`）

**修法**：本地构建脚本不导出这两个变量，让两趟都退回 `.cargo/config.toml`
的 `opt-level = "z"`。另外构建前要 `gradlew --stop`：Gradle daemon 是长驻 JVM，
环境在它启动那一刻就冻结了，历史上 export 过的值会一直传给
`android-studio-script` 那趟 cargo。

**顺带纠正一个常见误判**：仓库里确实有**两份内容相同**的 `.cargo/config.toml`
（根目录 + `src-tauri/`）。Cargo 的配置查找是「从 cwd 向上逐级读取并**合并**」，
两份都在时 `[target.aarch64-linux-android] rustflags` 这个**列表会被拼接两份**
（实测 `link-arg` 从 5 项变 7 项）。但两趟构建的 cwd 都是 `src-tauri/`
（① `build.rs` 里有 `set_current_dir(dirs.tauri)`；② `BuildTask.kt` 的
`workingDir = app/../../../`），**两趟读到的是同一组配置** ——
所以它**不是**重复编译的原因，删掉 `src-tauri/.cargo/config.toml` 也修不了这个问题。

#### 9.2.1 成因二：`data_manifest.json` 每次都被重写（**未修**）

上面那个成因修掉之后，构建**仍会**多编一次 `ling_chat`（约 6.5 分钟）。第二个成因是
**构建脚本自己踩了 cargo 的 `RerunIfChanged`**。

证据在 cargo 的 fingerprint 文件里
（`src-tauri/target/aarch64-linux-android/release/.fingerprint/ling_chat-*/run-build-script-build-script-build.json`）：

```json
"local": [{
  "RerunIfChanged": {
    "output": "...\\build\\ling_chat-*\\output",
    "paths": ["tauri.conf.json", "tauri.android.conf.json",
              "gen/android\\tauri.settings.gradle", "gen/android\\app\\tauri.build.gradle.kts",
              "capabilities", ".bundled_resources\\data_manifest.json"]   // ← 就是它
  }
}]
```

`.bundled_resources/data_manifest.json` 在 `RerunIfChanged` 列表里，而
`build-android-pet.sh` 的 `[6/7]`（`prepare-bundled-resources`）**每次构建都会重新生成
这个文件**。cargo 按 mtime 判定 → 构建脚本的输入变了 → `FsStatusOutdated` →
构建脚本重跑 → `ling_chat` 这个 crate 被判脏、重编。

**注意这不只是「多花 6.5 分钟」**：任何对 `data/` 的改动都会通过这条链触发 `ling_chat`
全量重编，和 Rust 源码有没有改无关。排查构建时间异常时容易误以为是增量缓存坏了。

**候选修法**（都没做，留待需要时）：

1. 让 `prepare-bundled-resources` 在内容**逐字节相同**时**不写文件**（先比对再决定是否落盘）；
2. 生成时先写临时文件再 `rename`，并显式把 mtime 设成固定值；
3. 构建前把 `.bundled_resources/data_manifest.json` 的 mtime 复位到某个稳定时间戳。

方案 1 最干净 —— 语义上「清单没变就不该算变更」，但它要改 `scripts/` 下的生成器，
超出本分支「悬浮窗」的范围，所以先记在这里。

### 9.3 为什么 Android 改动本地 `cargo check` 测不出来

Rust 侧本地 `cargo check`（host target）只能验证桌面端，
`#[cfg(target_os = "android")]` 的 `mobile.rs` 根本不参与编译。
只有真正做 `aarch64-linux-android` 交叉编译 + Kotlin 编译才覆盖得到——
`build-android-pet.sh` 的第 7 步做的就是这件事，因此**它才是有效验证**。

P0 阶段曾因此漏掉两个只有真机/交叉编译才能发现的错误
（`run_mobile_plugin` 的宿主类型、参数缺 `Serialize`）。

### 9.4 CI（备用）

```bash
gh workflow run dev-build-android.yml --repo zhangzm0/LingChat --ref feat/android-floating-pet
gh run download <run-id> --repo zhangzm0/LingChat -n lingchat-dev-android
```
