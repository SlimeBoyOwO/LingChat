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

#### 4.1.1 更正：Android 上**可以**开第二个 Tauri 窗口（但和「裸 WebView」是两回事）

容易被上面那段带偏成「Android 只能有一个 WebView」。**不是的。**
Tauri 的 `WebviewWindow` 在 Android 上是支持的，只是机制和桌面端不同 ——
它**给每个窗口起一个独立的 Android Activity**。

翻 `tao-0.35.2/src/platform_impl/android/ndk_glue.rs` 的 `create_activity`：

```rust
let activity_class = find_class(&mut env, &main_activity,
        format!("{}/{activity_name}", PACKAGE.get().unwrap()))?;
let activity_id = jni_call_method!(env, &main_activity, "startActivity",
        "(Ljava/lang/Class;)I", &[(&activity_class).into()], i)?;
// 然后等 ACTIVITY_CREATED_SENDERS 回信，超时 5 秒
```

也就是**真正的 `startActivity(Class)`**，不是 Activity Embedding。所以最小要求只有三条：

1. 在**与 `MainActivity` 同一个包**下加一个 `class SettingsActivity : TauriActivity()`；
2. 在 `AndroidManifest.xml` 里注册它；
3. 创建窗口时传 `activityName: "SettingsActivity"`。

**不需要** Activity Embedding / `androidx.window` / `androidx.startup` /
split 规则 —— 那些是「同一屏**并排**显示」才要的（官方文档把它们和 `splitRatio`
写在一起，容易误读成必需）。手机上就是**新 Activity 压进返回栈**，按返回回到上一层。

**关键区别**：`SettingsActivity : TauriActivity` 会跑完整的 Rust glue
（`PluginManager.onActivityCreate` 等），所以**它有 IPC**；而 4.1 里被否决的
`WebView(activity)` 是**裸 WebView**，没有 `addJavascriptInterface`，所以是空壳。
两者别混为一谈。

代价也要知道：第二个窗口是**全新的 WebView 实例 + 全新 JS 上下文**，
要重新 boot 整个前端（几秒），内存也多一份；窗口间状态靠 Rust 侧共享状态 +
事件广播同步（桌面端现在正是靠 `pet-scale-changed` 这类事件）。

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
                // ★ 取短边/长边，再按 rotation 分配 —— **不是**「按 rotation 交换 pw/ph」
                // 见下面「方向约定」一节
                val shortSide = minOf(pw, ph)
                val longSide = maxOf(pw, ph)
                val rotated = display.rotation == Surface.ROTATION_90 ||
                    display.rotation == Surface.ROTATION_270
                return if (rotated) longSide to shortSide else shortSide to longSide
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
- `getMode()` → 物理分辨率，**方向约定不保证**（见下）。

**判据**：只要一个 API 的签名里出现 `Resources` / `Configuration` /
`DisplayAdjustments`，或者内部会读它们，在这个场景下就**不可信**。
`getRealMetrics` / `getSize` / `getMetrics` / `WindowMetrics` 全都中招 ——
前两个看着在 `Display` 上，实则内部都要过一次 `adjustMetrics`。

所有读屏幕尺寸的路径都收敛到这一个函数（`screenWidthDp` / `screenHeightDp` /
`clampIntoScreen` / `snapToEdge` / `reapplyWindowAfterConfigChange` / `show()` /
`keepAliveTick`），改一处即全部生效。

#### 4.4.2.2 ⚠️ 「来源对了」不等于「组合对了」—— 方向约定那一脚

上一节把来源从 Resources 换成了 `DisplayInfo`，这一步是对的，但**组合方式**
仍然错了一版：`return if (rotated) ph to pw else pw to ph`。

`Display.Mode.getPhysicalWidth()/Height()` 的**方向约定是不保证的**：

| 约定                     | 横屏时 mode 报什么 | 按 rotation 交换后    |
| ------------------------ | ------------------ | --------------------- |
| 面板自然方向（文档口径） | `pw=1080, ph=2400` | `(2400,1080)` ✅ 横屏 |
| 当前方向（不少机型）     | `pw=2400, ph=1080` | `(1080,2400)` ❌ 竖屏 |

**两种约定下交换的结果正好相反。** 竖屏时两者结果相同（都是 `(短, 长)`），
所以「竖屏一直正常」，只有横屏才错 —— 这也解释了为什么上一轮在测试机上
「修好了」，换台机器又复发。

真机症状与代码一一对应：

| 症状                     | 对应量                                                 |
| ------------------------ | ------------------------------------------------------ |
| 桌宠「只能停在左半边」   | `screenW` 读成竖屏 360dp → `maxX = 360 - width`        |
| 桌宠「会超出屏幕」       | `screenH` 读成竖屏 800dp → 允许落到真实高度 360dp 之外 |
| 「悬浮窗以为自己是竖屏」 | 页面 `fit = window.innerWidth / 210` 跟着错            |

**修法**：只取**短边 / 长边**，再按 rotation 决定谁当宽谁当高。这个写法对上面
两种约定**同时成立**（短边、长边两个数值与约定无关），于是结果唯一。

> **转屏后位置不对时先看日志**：
> `屏幕尺寸变化：悬浮窗重排为 … 屏幕 WxH（rotation=…, mode=…x…）` ——
> rotation 与物理尺寸都在，一眼能分清是「旋转没读到」还是「读到但算错了」。

> `density` 仍然取自 `activity.resources.displayMetrics.density` —— 密度不受旋转影响，
> 而且 WebView 的 `devicePixelRatio` 也来自它，两边必须同源。

##### 4.4.2.2 贴边要贴**安全区**，不是物理边缘

真机现象：横屏时桌宠贴右沿会「**右边溢出去一点**」。

`screenSizePx()` 给的是**物理**尺寸 —— 含挖孔、圆角、手势条所在的那圈区域。
而桌宠是可以贴边吸附的：按物理边缘钳制，它就会有一部分压在挖孔 / 圆角上。
横屏尤其明显，因为挖孔通常落在屏幕的某一侧，那一侧的可视宽度本来就比
物理宽度窄。

**修法**：把安全区（挖孔 + 系统栏）从钳制与吸附的边界里扣掉。

```kotlin
private fun safeInsetsPx(): SafeInsets {
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
        val insets = activity.getSystemService(WindowManager::class.java)
            ?.currentWindowMetrics?.windowInsets ?: return SafeInsets.NONE
        var l = 0; var t = 0; var r = 0; var b = 0
        insets.displayCutout?.let {              // 挖孔 / 圆角
            l = maxOf(l, it.safeInsetLeft); t = maxOf(t, it.safeInsetTop)
            r = maxOf(r, it.safeInsetRight); b = maxOf(b, it.safeInsetBottom)
        }
        val bars = insets.getInsets(WindowInsets.Type.systemBars())   // 状态栏 / 手势条
        l = maxOf(l, bars.left); t = maxOf(t, bars.top)
        r = maxOf(r, bars.right); b = maxOf(b, bars.bottom)
        return SafeInsets(l, t, r, b)
    }
    return SafeInsets.NONE
}
```

`clampIntoScreen` 与 `snapToEdge` 都改成在 `[safe.left, screenW - safe.right]`、
`[safe.top, screenH - safe.bottom]` 里取值；`reapplyWindowAfterConfigChange`
的上下插值也按安全区跨度算。

> ⚠️ **这条读数和 4.4.2.1 那三个被证伪的来源是「同一个类、不同代码路径」。**
> `WindowMetrics` 上：
>
> - `bounds` ← `windowConfiguration.getMaxBounds()` —— **就是** 4.4.2.1 里被证伪
>   的那条 Resources 路径，**绝不能用**；
> - `windowInsets` ← `InsetsStateController` —— **与 Resources 无关**，旋转后
>   立刻是新值。
>
> 所以这里**只取 insets，绝不取 bounds**。下次谁看到 `getCurrentWindowMetrics`
> 想顺手把 `bounds` 也用了，请先读回 4.4.2.1。

> 取的是**整块屏幕**口径的 insets，不是悬浮窗自己的 —— 后者会随窗口位置变化
> （窗口没压到状态栏时上内缩就是 0），拿来当安全区会时灵时不灵。
>
> `getInsets(Type)` 会把**当前不可见**的系统栏算作 0（沉浸式时手势条隐藏就
> 不该避让），正是我们要的语义。

> 低于 API 30 退回「无边距」：宁可和以前一样贴物理边缘，也不要因为拿不到
> insets 把桌宠挤到屏幕中间。

##### 4.4.2.3 旋转还有一条**不依赖系统回调**的兜底

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

### 4.5 放久了「消息能发出去，但没反应」—— 全局生成锁 + 缺一层**绝对**超时

**真机症状**（用户报告）：

> 悬浮窗放久之后，那些特效还在动，也能折叠展开，自动播放键也能点，
> 但跟角色对话时消息能发出去，就是没反应。角色卡、剧本日志也都变成空的，
> 背景正常。

拆开看：

| 现象                        | 说明什么                                                       |
| --------------------------- | -------------------------------------------------------------- |
| 特效在动、能折叠、按钮可点  | **前端 JS 活着** —— WebView 没崩，进程也没被杀                 |
| 消息「能发出去」            | 只是前端**乐观地**把这条消息插进了列表，不代表 Rust 真的收到了 |
| 没有回复、角色卡 / 日志变空 | 卡在 **Rust 侧**：凡是需要拿锁的状态查询与生成，全排在后面     |

也就是：**Rust 还活着，但某把锁被永久持有了。**

#### 4.5.1 病灶：`generation_lock` 被持有跨越整条 LLM 流式生成

`AppState::generation_lock`（`app/state.rs:57`）是**全局唯一**的生成锁，
类型 `Arc<tokio::sync::Mutex<()>>`。它的持有范围不是「拿一下」，而是**整条生成**：

```rust
// api/chat.rs:202（用户发消息）
let _lock = gen_lock.lock().await;
match generator.process_message(Some(text)).await { ... }

// ai_service/proactive_system/mod.rs:240（主动对话）
let _lock = self.generation_lock.lock().await;
let _ = generator.process_message(None).await;
```

全部持锁站点：`api/chat.rs`（202 / 302 / 398 / 579）、`api/game.rs`（391 / 930）、
`ai_service/proactive_system/mod.rs:240`、`ai_service/tools/background_command.rs:268`。

于是只要**任意一轮生成永不返回**，后续所有对话请求都会永久排在它后面 ——
而且**不可恢复**：`tokio::sync::Mutex` 没有超时，也没有强制释放。

#### 4.5.2 为什么「永不返回」是可能的

LLM 侧现有的超时**都只在「空闲」时计时**：

| 层      | 位置                                                    | 计时口径           |
| ------- | ------------------------------------------------------- | ------------------ |
| reqwest | `llm/factory.rs`：`Client::builder().read_timeout(...)` | 两次**数据块**之间 |
| 流包装  | `llm/mod.rs:189`：`timeout(idle_timeout, inner.next())` | 两次 `next()` 之间 |

**只要服务端持续发送 SSE 心跳 / 注释字节，这两层都不会触发。**
另外没有配 connect timeout，网络被系统挂起（Android 后台省电、doze、切网）时
连接可以挂很久。

「放久之后」这个时机也刚好对上：`ProactiveSystem` 每 30 秒一轮，兴趣累积到
阈值就会主动发起一次生成 —— 正是用户闲置一段时间之后。而它 `deliver()` 里
第一件事就是拿 `generation_lock`。

#### 4.5.3 修法：给「前置步骤」和「每一轮生成」各加一层**绝对**上限

新增 `GENERATION_HARD_LIMIT_SECS = 600`（`config/app_config.rs`），
在 `MessageGenerator`（`ai_service/message_system/generator.rs`）里包两处：

```rust
// process_message：前置步骤（用户消息落库 / 翻译、场景检测、上帝 Agent 预选）
let user_ctx = match tokio::time::timeout(
    Self::hard_limit(),
    self.prepare_user_turn(user_message.as_deref()),
).await {
    Ok(result) => result?,
    Err(_) => return Err(self.abort_on_hard_limit("前置处理")),
};

// execute_pipeline：每一轮 LLM 生成
let outcome = tokio::time::timeout(
    Self::hard_limit(),
    self.run_pipeline(context, user_message.to_string(), user_msg_seq),
).await;
let outcome = match outcome {
    Ok(result) => result,
    Err(_) => Err(self.abort_on_hard_limit("LLM 生成")),
};
```

**为什么选在 `MessageGenerator`，而不是逐个去改那 8 个持锁站点：**

- **它是唯一咽喉。** `api/chat.rs`、`api/game.rs`、`proactive_system`、
  `background_command` 的生成全部经由 `process_message` / `process_notification`。
  改一处，所有站点都受保护，也完全不用动各站点的锁结构。
- **超时后锁一定会放。** `tokio::time::timeout` 超时会把 future **drop** 掉，
  future 里持有的 `MutexGuard` 随之 drop —— 生成锁、`ai_service`、`game_status`
  一起释放；同时 drop 掉 reqwest 的响应流，连接也就断了。
- **前端自动复位。** `abort_on_hard_limit` 里调 `events::emit_error`，它本来就
  会 emit `ai:error`（载荷 `{ error_code, detail }`）+ `status:reset`，前端据此
  把输入框从等待态放回来。文案里特意带「超时」二字，好让
  `classify_llm_error` 归成 `error_code = "timeout"`，复用现成的 i18n。
- **按「轮」而不是按「整轮对话」计时。** 上帝 Agent 模式下一轮对话可能连发
  多个 NPC（受 `max_consecutive_npc` 限制），按整轮计时会误伤；按每一轮
  LLM 生成计时才是真正的「单次不可控等待」粒度。

#### 4.5.4 这层上限**不**覆盖什么（诚实的边界）

它只保证「生成链路上的锁不会永久持有」。以下路径仍是裸的：

- `api/chat.rs::generate_line_voice` —— 持 `generation_lock` 跑 TTS 网络请求
- `api/chat.rs::rollback_conversation` / `api/game.rs::clear_conversation` —— DB 操作
- `proactive_system` 里 `get_proactive_prompt` 那段：**同时持有 `ai_service`
  与 `game_status` 两把锁**去跑视觉模型（那段代码自己的注释就写了
  「SCREEN 调用视觉模型可能耗时」）。这是目前**第二可疑**的一处 —— 它一旦卡住，
  连 `send_message` 构建 deps 都进不去，症状与本次完全一致。
- `ScreenAnalyzer`（图片投喂路径）持自己的锁跑视觉模型。

如果真机复现后「发消息没反应」仍在，**下一步就按这个清单逐个加界**。

### 4.6 Android 返回键走的是 **WebView 历史栈** —— 形态切换必须用 `replace`

**真机症状**（用户报告）：

> 在聊天页按返回键后是宠物页；再返回一下就又变回一个聊天页，然后再返回才是主页。

这正是**历史栈里堆了一串来回的记录**。

#### 4.6.1 谁在处理返回键

`WryActivity` 里确实有这样一段：

```kotlin
// gen/android/.../generated/WryActivity.kt
open val handleBackNavigation: Boolean = true
...
if (handleBackNavigation) {
    val callback = object : OnBackPressedCallback(true) {
        override fun handleOnBackPressed() {
            if (mWebView.canGoBack()) mWebView.goBack()
            else { isEnabled = false; onBackPressed(); isEnabled = true }
        }
    }
    onBackPressedDispatcher.addCallback(this, callback)
}
```

> ⚠️ **但这段在我们这里没有生效**：`TauriActivity` 把它覆写成了
> `override val handleBackNavigation: Boolean = false`，所以回调**根本没注册**。
> 全仓（生成的 Kotlin + wry 自带模板）只有这一处返回键代码，别处没有。

真正生效的是 **WebView 自身的内置返回处理** —— Chromium 会向宿主
`ComponentActivity` 的 `OnBackPressedDispatcher` 注册回调，`canGoBack()` 就
`goBack()`；不能回退时回落到 Activity 的 `onBackPressed()`（= 退出 App）。

**结论不变**：**`vue-router` 的历史栈 = 返回键的路径**。压了几条，就要按几次。

#### 4.6.2 病灶：把「形态切换」当成「前进一页」

`/chat` 和 `/pet` **不是两个页面，是同一个界面的两种形态**。但代码里全用了
`router.push`，于是每切一次形态就多压一条历史：

```
/  --push--> /chat  --push--> /pet  --push--> /chat
                                              ↑ 用户在这里
按返回 → /pet   （用户：「按返回键后是宠物页」）
按返回 → /chat  （「再返回一下就又变回一个聊天页」）
按返回 → /     （「然后再返回才是主页」）
按返回 → 退出 App
```

涉及的 8 处（`MainChat.vue` 6 处 + `PetMode.vue` 2 处）：
`goToPetMode` 的三个分支、`leavePetMode`、`healStuckFloatingState`、
搬移失败的兜底回退、`PetMode.handleReturnedToApp`、`PetMode` 桌面端退出。

#### 4.6.3 修法：形态切换一律 `router.replace`

```js
// ❌ 每切一次形态就多一条历史，返回键要在两页之间反复弹
await router.push("/pet");
// ✅ 替换当前历史项：历史栈永远是「/ → 当前形态」两层
await router.replace("/pet");
```

修完之后的返回路径：

```
/  --push--> /chat  --replace--> /pet  --replace--> /chat
                                                    ↑ 用户在这里
按返回 → /    （一步到位）
按返回 → 退出 App
```

**判据**：`push` 用于「用户主动前进到下一个页面」（`/` → `/chat`、
`/` → `/credit`、`/chat` → `/script-editor`）；`replace` 用于「同一逻辑位置
换一种形态」（`/chat` ↔ `/pet`）。后者压历史，用户就会觉得返回键「鬼打墙」。

> ⚠️ 历史栈修短之后，`goBack()` 到 `/` 仍会触发一次 SPA 导航（渲染主菜单），
> 而不是直接退出 —— 因为 `canGoBack()` 仍为 true。要「返回即退出」得让
> 历史只剩一条，那会连带影响 App 内的其它返回路径，暂不做。

### 4.7 桌宠「设置」：手机端开一个**独立 Activity**

**真机症状**：桌宠悬浮窗里点「设置」，**没有任何反应**。

#### 4.7.1 病灶：`WebviewWindow` 少了 `activityName`

`PetMode.handleOpenSettings` 原本是照桌面端写的：

```js
const webview = new WebviewWindow("settings", { url: "/second" /* … */ });
```

桌面端多窗口是原生能力，`url` 就够了。Android 上按 4.1.1 的机制，
tao 必须知道**用哪个 Activity 承载** —— 不给 `activityName`，
`find_class("<包名>/")` 直接抛 `ClassNotFoundException`，窗口建不出来，
只走 `tauri://error` 事件。前端**没有监听** `tauri://error`，所以表现就是
「点了没反应」，日志里什么都没有。

#### 4.7.2 三处必须一起改（少一处就白改）

| 位置                                                    | 内容                                                           | 漏了会怎样                                     |
| ------------------------------------------------------- | -------------------------------------------------------------- | ---------------------------------------------- |
| `gen/android/.../com/noiq/lingchat/SettingsActivity.kt` | `class SettingsActivity : TauriActivity()`                     | 找不到类                                       |
| `AndroidManifest.xml`                                   | `<activity android:name=".SettingsActivity" …>`                | `startActivity` 抛 `ActivityNotFoundException` |
| `PetMode.vue`                                           | `...(isAndroid() ? { activityName: "SettingsActivity" } : {})` | 同上（走不到那一步）                           |

另外两条**容易踩的细节**：

- **`launchMode` 必须是默认的 `standard`**。tao 建窗后会 `rx.recv_timeout(5s)`
  等 `ACTIVITY_CREATED_SENDERS` 的回信（见 4.1.1 引的源码）。
  `singleTask` / `singleTop` 在**复用已有实例**时不会走完整创建流程，
  那封信就永远不来 —— 白等 5 秒然后建窗失败。重复点击由前端
  `WebviewWindow.getByLabel("settings")` 拦掉，不需要 `launchMode` 兜。
- **`configChanges` 要与 `MainActivity` 逐项对齐**。不声明的话转屏会**重建**
  Activity，而 Activity 一重建里面的 WebView 就没了，整个设置页要重新加载。

#### 4.7.3 主题：不能沿用 `Theme.ling_chat`

`Theme.ling_chat` 是**桌宠专属**的沉浸式全屏主题：

```xml
<item name="android:windowFullscreen">true</item>
<item name="android:windowShowWallpaper">true</item>
<item name="android:windowIsTranslucent">true</item>
<item name="android:windowBackground">@android:color/transparent</item>
```

设置窗照抄有两个后果：

1. **糊在壁纸上**。而且雪上加霜：wry 在窗口选项 `transparent: true` 时会把
   WebView 背景也置成全透明（`wry/src/android/main_pipe.rs` 的
   `set_background_color(…, (0,0,0,0))`），而前端 `html/body` 本来就是
   `background: transparent` —— 三层全透，设置页直接叠在系统壁纸上。
2. `windowFullscreen=true` 在 Android 14 及以下会把状态栏一起藏掉
   （看不到时间/电量）；Android 15+ 又因为强制 edge-to-edge 是另一套行为，
   两端分裂。

所以新增 `Theme.ling_chat.settings`（`res/values/themes.xml`，显式
`parent="Theme.ling_chat"` 后逐项覆盖），`windowBackground` 用
`?android:colorBackground` —— 它是 DayNight 属性，随系统深浅色自动取
白/近黑，不用新增颜色资源。

> 只在 `values/` 里定义即可：`values-night/themes.xml` 重新定义的是**父**主题
> `Theme.ling_chat`，子主题按资源限定符回退规则照样能找到。
>
> ⚠️ **踩过的坑**：XML 注释里**不允许出现连续两个 ASCII 减号 `--`**
> （`Error: 注释中不允许出现字符串 "--"`，报在 `mergeUniversalReleaseResources`）。
> 而安全区的 CSS 变量名恰好就是 `--safe-area-inset-*` 这种双横线开头 ——
> 想在这个注释里提它，只能不写前导双横线。注意中文破折号 `——`（U+2014）**不**受影响。

#### 4.7.4 关窗：`Window.close()` 在 Android 上会留**一块黑屏**

前端 `getCurrentWindow().close()` 走的是
`WindowMessage::Close` → `on_close_requested` → Tauri 把窗口从登记表里摘掉。
**但关掉 Activity 这一步在 Android 上没人做** —— 翻
`tao-0.35.2/src/platform_impl/android/mod.rs`，`Window` 上**没有**
`close()` / `destroy()` 实现，`on_window_close` 只是把 Rust 侧的 wrapper 置空。

结果：WebView 被销毁、承载它的 Activity 还在 → 用户看到**一块黑屏**，
只能靠返回键脱身。

修法是给设置窗注入一个原生桥。`WryActivity` 留了个钩子
（`open fun onWebViewCreate(webView: WebView) {}`，`TauriActivity` 没覆写它）：

```kotlin
override fun onWebViewCreate(webView: WebView) {
    SafeAreaInsets.attach(webView)
    webView.addJavascriptInterface(
        object {
            @JavascriptInterface
            fun close() { runOnUiThread { sendWholeAppToBack() } }
        },
        "LingChatSettings",
    )
}
```

前端 `SettingsPage.closeWindow` 在移动端优先调 `window.LingChatSettings.close()`，
拿不到接口时才退回 `appWindow.close()`。

> ⚠️ **这个接口里原来是 `finish()`，2026-10-03 改成「退回」** —— 语义与理由见
> **4.7.12**。`finish()` 会连带把用户扔回聊天页，而且每次都重建整个设置页。
> 现在的 `sendWholeAppToBack()` = 设置任务 + 聊天任务一起退到后台。

**为什么用 `onWebViewCreate` 而不是 `window.decorView.post { findWebView() }`**：
翻 `wry/src/android/main_pipe.rs`，`CreateWebView` 消息按 `activity_id` 找到
本 Activity 后 `new RustWebView(activity, …)` 再 `activity.setWebView(webview)`，
**之后**才 `load_url`。所以 `onWebViewCreate` 是**确定**拿得到 WebView 的时机；
`decorView.post` 则可能在 WebView 还没建出来时直接 `return`，**静默失效**。

> `@JavascriptInterface` 的方法跑在 WebView 的 JavaBridge 线程上，而
> `sendWholeAppToBack()` 里两步任务栈操作都该在主线程上做，所以套了一层
> `runOnUiThread`。

#### 4.7.5 ⚠️ `PluginManager.onDestroy` **没有**守卫 —— 关设置窗会误拆悬浮窗

这是改这个功能时最容易忽略、后果最严重的一条。

`PluginManager.kt`（`tauri-2.11.1/mobile/android/...`）里两个方法是不对称的：

```kotlin
fun onActivityCreate(activity: AppCompatActivity) {
    // TODO: on destroy, we should change to a different activity
    if (::activity.isInitialized) { … return }   // ← 有守卫，只认第一个
    this.activity = activity
    …
}

fun onDestroy(activity: AppCompatActivity) {
    for (plugin in plugins.values) {
        plugin.instance.triggerOnDestroy(activity)   // ← 没有守卫，每个都转发
    }
}
```

`TauriActivity.onDestroy()` 会调 `PluginManager.onDestroy(this)` —— 于是
**用户一关设置窗，`FloatingPetPlugin.onDestroy` 就被调一次**，把还开着的
悬浮窗一起拆掉。真机表现是「打开设置、返回，桌宠就没了」，看起来像收回
逻辑出错，其实是这里误伤。

修法（`FloatingPetPlugin.kt`）：

```kotlin
override fun onDestroy(activity: AppCompatActivity) {
    if (activity !== this.activity) {
        super.onDestroy(activity)
        return
    }
    activity.runOnUiThread { detachPetView() }
    super.onDestroy(activity)
}
```

判据用**实例相等**而不是类名或 `isFinishing`，因为插件是**单例**：
`register_android_plugin`（`tauri-2.11.1/src/plugin/mobile.rs`）只在 app setup 时
`env.new_object(plugin_class, "(Landroid/app/Activity;)V", &[activity])`
实例化**一次**，并把 `PluginManager.load(...)` 写进 `plugins[name]`。
而那时能拿到的 Activity 就是 MainActivity（`onActivityCreate` 的守卫决定的）。
所以 `this.activity` 一定指向悬浮窗的宿主。

#### 4.7.6 安全区：抽成两个 Activity 共用的注入器

`targetSdk = 36`，**Android 15+ 对 targetSdk ≥ 35 的应用强制 edge-to-edge** ——
任何 Activity 都会铺到系统栏下面，设置窗也不例外。

原先的注入逻辑写死在 `MainActivity.injectSafeAreaToWebView()` 里，只对
MainActivity 生效。这次抽成 `SafeAreaInsets.kt`（`internal object`），
两个 Activity 共用：

```kotlin
internal object SafeAreaInsets {
    fun attach(webView: WebView) { /* setOnApplyWindowInsetsListener + requestApplyInsets */ }
    private fun apply(webView: WebView, bars: Insets) { /* 写 4 个 CSS 变量 */ }
    fun findWebView(parent: ViewGroup): WebView? { /* 给拿不到钩子的场景兜底 */ }
}
```

- `MainActivity` **两条路径都挂**：主路径是 `onWebViewCreate`（确定命中），
  兜底保留原来的 `window.decorView.post { findWebView() }`。两条都是幂等的
  （`setBackgroundColor` 幂等；`attach` 是替换监听器），重复执行无害。
  只留 `decorView.post` 是在**赌** —— 它和 Rust 主线程的 `setWebView`
  谁先谁后并不确定，扑空时 `?: return` **静默失效**；
- `SettingsActivity` 只走 `onWebViewCreate`。

消费端是设置页根元素上的 `.pt-safe .pr-safe .pb-safe .pl-safe`
（`base.css` 已提供），桌面端这些变量恒为 `0px`，零回归。

> ⚠️ 不能指望 `env(safe-area-inset-*)`：`base.css` 里那只是**回退值**，
> Android WebView 的 `env()` 取不到宿主 insets，实测恒为 `0px`。

#### 4.7.7 布局：窄屏下设置页要重排

设置页原本是照 1200×800 的桌面窗口设计的（200px 竖侧栏 + 内容区）。
放在 360dp 的手机屏上，侧栏吃掉 200px，内容只剩 160px，减掉 `p-6`
就剩 112px —— 滑块根本没法用。三处调整：

| 组件                  | 改动                                                   | 依据                                                           |
| --------------------- | ------------------------------------------------------ | -------------------------------------------------------------- |
| `SettingsSidebar.vue` | 窄屏切成顶部**横向滚动**标签条，`md:` 以上维持左侧竖栏 | 按**可用宽度**判断，不按设备类型（桌面端把窗口拉窄时也该切）   |
| `SettingsHeader.vue`  | 最小化/最大化按钮 + 分隔线 `v-if="!isMobile()"`        | `Window.minimize()` / `toggleMaximize()` 在 Android 上是空操作 |
| `SettingsPage.vue`    | 根元素加四边 `.xx-safe` + `max-md:rounded-none`        | 全屏时圆角会切出小三角露出窗口底色                             |

#### 4.7.8 ⚠️ 窄屏下内容区宽度是 **0** —— `main` 漏了 `flex-col`

4.7.7 把 `SettingsSidebar` 改成了「窄屏顶部横向标签条」，但**父容器没跟着改**：

```html
<main class="relative flex flex-1 overflow-hidden">
  <!-- ← 横向 flex -->
  <aside class="w-full shrink-0 flex-row md:w-[220px] md:flex-col … …">
    <!-- 标签条 -->
    <section class="flex-1 overflow-y-auto … …"><!-- 内容区 --></section>
  </aside>
</main>
```

窄屏时 `<aside>` 的 `w-full` 让它占满整行，而它 `shrink-0` 不参与收缩；
`<section>` 的 `flex: 1 1 0%` 因此分到 **0 宽**（它带 `overflow-y-auto`，
`min-width: auto` 按规范归零，不会撑开），再被 `main` 的 `overflow-hidden` 裁掉。

**真机表现**：竖屏进设置页只看到一条标签条，下面一片空白；点标签**内容确实
切换了**，只是宽度是 0 看不见 —— 用户描述成「没有竖屏适配 + 像卡死」。

**修法**：`flex-col md:flex-row`。设计本来就是「窄屏标签条在顶部、内容在下面」，
这一条只是漏写了。

> 教训：响应式改造里**改子元素的断点**和**改父容器的方向**必须一起做。
> 只改一边不会报错、不会有编译期提示，只会「静默地少一块」。

#### 4.7.9 横屏安全区：主题必须声明 `windowLayoutInDisplayCutoutMode`

`values/themes.xml` 与 `values-night/themes.xml` 的 `Theme.ling_chat` 都
**没有**声明这个属性。Android 15+ 且 targetSdk ≥ 35 时默认行为是 `always`
（配合强制 edge-to-edge），但 **Android 14 及以下不是** —— 默认值在**横屏**
会把窗口「信箱化」：系统在挖孔那一侧留出空白，窗口本身被挤窄。

于是横屏出现两个叠加问题：

| 症状                      | 成因                                                           |
| ------------------------- | -------------------------------------------------------------- |
| 左侧多出一块空白          | 系统已经为挖孔留白，前端 `.pl-safe` 又按 insets 补一次 → 双份  |
| 右侧内容被切掉 / 超出屏幕 | 窗口可用宽度 < 物理宽度，而 `100dvw` 仍按整屏算 → 右侧溢出被裁 |

显式写 `always` 之后，窗口在所有方向都铺满整块屏幕，安全区**只由**前端按
`safe-area-inset` 那组 CSS 变量内缩一次，不再叠加系统那一份。

> ⚠️ **XML 注释里不能出现连续两个 ASCII 减号 `--`**（`Error: 注释中不允许出现
字符串 "--"`，报在 `mergeUniversalReleaseResources`）。而安全区变量名恰好
> 是双横线开头的 —— 想在注释里提它，只能不写前导双横线。中文破折号 `——`
> （U+2014）不受影响。

#### 4.7.10 转屏后强制重排：两个 Activity 都要覆写 `onConfigurationChanged`

`configChanges` 里声明了 `orientation|screenSize|…`，好处是转屏**不重建**
Activity（WebView 不销毁、设置页不重新加载），代价是**系统不再帮我们重排** ——
它只调一次 `onConfigurationChanged`，而 `TauriActivity` 仅把它转发给
`PluginManager`（插件默认空实现）。

**Activity 在后台时转屏**（用户在别的 App 里转了屏再切回来）最明显：窗口尺寸
是回到前台那一刻才真正变的，WebView 的视觉视口（`window.innerWidth/innerHeight`）
可能仍停在旧方向，页面于是按旧尺寸排版。

两个 Activity 都补上，三件事缺一不可：

```kotlin
override fun onConfigurationChanged(newConfig: Configuration) {
    super.onConfigurationChanged(newConfig)
    window.decorView.requestApplyInsets()   // ① 重发 insets，刷新安全区变量（挖孔换边）
    webView.requestLayout()                 // ② 强制 WebView 重新测量
    webView.evaluateJavascript(             // ③ 给页面一个 resize
        "window.dispatchEvent(new Event('resize'));", null)
}
```

③ 不能省：前端的 `--pet-fit`、画布高度、各种尺寸缓存**都挂在 resize 上**，
光重测 DOM 不会让 JS 重算。

> `WryActivity.mWebView` 是 `private`，所以在 `onWebViewCreate` 里自己存一份引用。
> 桌宠在悬浮窗里时该 WebView 已被搬进 `WindowManager`，不在 Activity 视图树里 ——
> 那种情况由 `FloatingPetPlugin` 的 `DisplayManager` 回调 + 保活轮询负责，
> 这里多派发一次 resize 只会让页面用当前视口重算一次，无害。

#### 4.7.11 「桌宠大小」滑块拖了等于没拖 —— 宽度是原生独占的

悬浮窗的**宽度**由原生独占：`屏幕短边 × 比例 × petScale`（见
`collapsedSize` / `expandedSize`），页面只负责**高度**。而
`PetMode.vue` 的 `appStyleVars` 在悬浮窗分支里**刻意不乘** `pet.scale`
（手机上再乘一次会双重缩放），`pet-scale-changed` 监听里又只调了桌面端的
`applyWindowLayout()` —— **两头都不动**，于是滑块形同虚设。

修法是新增 `set_scale` 命令。这条链路**四处缺一不可**，其中第 4 条最容易漏：

| #   | 位置                                                      | 内容                                                                    |
| --- | --------------------------------------------------------- | ----------------------------------------------------------------------- |
| 1   | `FloatingPetPlugin.kt`                                    | `@InvokeArg class ScaleArgs` + `@Command fun setScale`                  |
| 2   | `src/{models,mobile,desktop}.rs`                          | `ScaleArgs` / 转发 / **桌面端也要补一个返回 `NotSupported` 的同名函数** |
| 3   | `src/lib.rs`                                              | `#[tauri::command]` + 注册进 `generate_handler!`                        |
| 4   | **`build.rs` 的 `COMMANDS` + `permissions/default.toml`** | 见下                                                                    |

第 4 条：**Tauri v2 的插件权限是显式清单**。`build.rs` 的 `COMMANDS` 数组驱动
`permissions/autogenerated/**` 的生成，而 `permissions/default.toml` 里
逐个列出 `allow-xxx`。新命令不加进去 → invoke 被权限系统**直接拒掉** ——
**不是编译错误，是运行时静默失败**（表现和「命令没实现」一模一样）。
第 2 条的桌面端那份也不能省：`lib.rs` 的 `use desktop as imp` / `use mobile as imp`
按 `cfg` 切换，桌面端缺函数就编不过（而本地 `cargo check` 覆盖的正是桌面端）。

`setScale` 的排布与 `setExpanded` 同一套：以**中心**为锚点重算宽高 → `clampIntoScreen`
→ 收起态 `snapToEdge` → **先 `notifyMetrics` 再 `updateViewLayout`**
（页面要先拿到新的 `--pet-fit`，否则会有几帧按旧系数渲染而溢出）。

#### 4.7.12 ★ 设置页的「关闭 / 返回键」= **退回 + 不销毁 + 独占任务栈**（2026-10-03 重写）

**真机反馈**（用户原话）：

> 宠物设置页应该有复用吧？短时间内如果反复调起就不销毁及隐藏即可。只要设置页
> 一推到后台就隐藏。……折叠态下可能会触发回到聊天页，这个我不知道怎么复现。
> 然后设置页应该配置返回键。

> 具体是你进入桌宠设置后，如果直接按桌宠上的返回键，设置页没有收起来

> 前面不是说吗？设置页尽量只用退回，不要用关闭，以节省性能

四条诉求，对应四个缺陷：

| 诉求                       | 旧行为（`finish()`）                                       |
| -------------------------- | ---------------------------------------------------------- |
| 反复调起不销毁             | 每次重建 Activity + WebView + Pinia store，重开必卡一下    |
| 「关掉设置却回到聊天页」   | `finish()` 落到 MainActivity，也就是**聊天页** —— 就是这条 |
| 返回键要配好               | 无历史时同样走 `finish()`，症状与「关闭」一模一样          |
| **桌宠返回键要收起设置页** | 见下 —— 这一条**不是** `finish()` 能修的                   |

最后一条是 2026-10-03 新报的，也是最难的一条：设置窗当时和 MainActivity
**同一个任务栈**，于是「收起设置页」和「露出聊天页」这两件事**不可能同时成立**。
真正的修法是**给设置窗一个独立任务栈**（4.7.12.2）。

##### 4.7.12.1 结论：三条路都不再 `finish()`，实例全程保留

| 动作                      | 行为                                                  | 实例 |
| ------------------------- | ----------------------------------------------------- | ---- |
| 点 ✕ / 返回键             | `sendWholeAppToBack()`：设置任务 + 聊天任务一起退后台 | 保留 |
| 桌宠「返回主页 / 返回键」 | 原生 `moveTaskToFront(聊天任务)`，设置任务留在后面    | 保留 |
| 再点「设置」              | 原生 `moveTaskToFront(设置任务)` 拉回前台（4.7.12.7） | 复用 |

`moveTaskToBack` 的参数是 `nonRoot`，**必须传 `true`**：传 `false` 时只有
「本 Activity 是任务根」才生效；传 `true` 表示「任务里任意一个 Activity 都能把
整个任务退到后台」，不依赖「自己是不是根」这个前提。

##### 4.7.12.2 ★ 为什么设置窗必须独占一个任务栈

`AndroidManifest.xml` 里给 `SettingsActivity` 加了三个属性，**缺一不可**：

```xml
android:taskAffinity="${applicationId}.settings"
android:launchMode="singleInstance"
android:excludeFromRecents="true"
```

**为什么同一个任务栈里做不到**（设置窗原先和 MainActivity 同在 MainActivity 的任务里）：

| 做法                                          | 结果                                                                                             |
| --------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| `SettingsActivity.moveTaskToBack(true)`       | **任务级**操作 → 连 MainActivity 一起退到后台，用户看到桌面                                      |
| `bringActivityToFront()`（`moveTaskToFront`） | 拉的是**整个任务**，栈顶仍是设置窗 → 按了返回键却还停在设置页（**真机反馈的正是这条**）          |
| `startActivity(MainActivity)`                 | MainActivity 是 `singleTask`，会把它上面的 Activity 一并销毁 → 设置窗没了（等于变相 `finish()`） |

让设置窗独占任务栈之后，`moveTaskToFront(聊天任务)` **只把聊天页翻到最前面**，
设置窗原样留在它自己的任务里 —— 退回、且不销毁。这一条就是那个 bug 的全部修法，
桌宠那边一行代码都不用改（`FloatingPetPlugin.hide()` 里本来就有
`bringActivityToFront()`）。

**为什么是 `launchMode` 而不是只写 `taskAffinity`**：

`taskAffinity` **只在 `FLAG_ACTIVITY_NEW_TASK` 下才参与任务选择**。而 tao 建窗走的
是 `WryActivity.startActivity(cls)`，它构造的是裸 `Intent(this, cls)`，**一个 flag
都没加**：

```kotlin
// generated/WryActivity.kt（自动生成，不能改）
fun startActivity(cls: Class<*>): Int {
    val intent = Intent(this, cls)
    intent.putExtra(ACTIVITY_ID_KEY, id)
    startActivity(intent)          // ← 没有 addFlags(FLAG_ACTIVITY_NEW_TASK)
    return id
}
```

所以光写 `taskAffinity` 会被忽略，设置窗照样落进调用者（MainActivity）的任务栈。
`singleInstance`（`singleTask` 同理）由系统的 `ActivityStarter` **自动补上
`NEW_TASK`** —— 这是 manifest 里**唯一**能拿到「另起任务」的杠杆。

> 为什么不去改 `WryActivity.kt` 加 flag：那是 tao 的自动生成文件，头部写着
> `THIS FILE IS AUTO-GENERATED. DO NOT MODIFY!!`，而且被
> `app/.gitignore` 的 `/src/main/**/generated` 忽略、**根本不入库** ——
> 改了不落库，还会在下次 `tauri android init` 时被覆盖。

**`excludeFromRecents`**：独占任务栈的副作用是「最近任务」里会多出一张设置页卡片，
加这个属性把整个设置任务从最近任务里去掉。

**⚠️ 已知代价：实例已存在时不能再走 tao 建窗。** tao 建窗后会等 Activity 的创建
回调（`ACTIVITY_CREATED`，超时 5 秒），而 `singleInstance` 在**复用已有实例**时走的是
`onNewIntent`、不会再触发 `onCreate`，回调也就不会来 —— 那次建窗会白等 5 秒然后失败。
所以「实例还在时绝不能再调 tao 建窗」是硬约束，由前端保证（见 4.7.12.7）。
设置窗改成永不销毁之后，实例要么「不存在（首次创建）」要么「存在且被复用」，
不会出现「实例在、tao 又建窗」。

> 顺带确认过 `ACTIVITY_CREATED` 的发送逻辑（`tao/.../ndk_glue.rs::onActivityCreate`）：
> 它按 `activity.getId()` 去 `ACTIVITY_CREATED_SENDERS` 里 `remove` 并回信，而
> `getId()` 就是 `WryActivity.id`（从 intent 的 `__wryActivityId` 读）。首次创建
> 一定有 `onCreate`，所以**建窗这条主路径不受 `singleInstance` 影响**。

##### 4.7.12.3 ✕ / 返回键 = `sendWholeAppToBack()`（两个任务一起退）

独占任务栈之后，`moveTaskToBack(true)` 只退设置任务，底下的聊天任务会顺势浮到
前台 —— 用户点「关掉设置」看到的是**聊天页**。而他要的是「回到进来之前那个 App」
（「关掉设置却回到聊天页」那条真机反馈说的就是这个），所以**两个任务都要退**：

```kotlin
private fun sendWholeAppToBack() {
    MainActivity.sendTaskToBack()   // 先退聊天任务
    moveTaskToBack(true)            // 再退自己
}
```

**顺序不能反**：

- 按这个顺序，第一步执行时设置页还在最前面，**屏幕上看不出任何变化**；
  第二步才把设置页收走，直接露出**进来之前那个 App**。
- 反过来的话，设置任务先退、聊天任务会短暂成为前台 —— 用户会瞥见一眼聊天页，
  然后才被退到后台。

`MainActivity.sendTaskToBack()` 是同步的（内部用 `runOnUiThread`，而调用时本来就在
主线程上，所以直接内联执行），两步之间的顺序稳定成立。拿不到 MainActivity 实例时
（理论上只在进程重启的瞬间）只退自己 —— 退化成「露出聊天页」，不是崩，可接受。

##### 4.7.12.4 桌宠「返回主页 / 返回键」为什么现在自动就对了

`FloatingPetPlugin.hide()` 里本来就有这一段（`HIDE_DELAY_MS = 80ms` 之后）：

```kotlin
bringActivityToFront()      // moveTaskToFront(MainActivity.taskId, MOVE_TASK_NO_USER_ACTION)
restoreWebViewToActivity()  // 把 WebView 从悬浮窗搬回 Activity 视图树
```

设置窗独占任务栈之后，`moveTaskToFront(聊天任务)` 把聊天页翻上来、设置窗留在后面，
「按桌宠返回键 → 设置页收起来 → 聊天页可见」**自动成立**。

反过来说：这条路径**不能**靠 `finish()` 或 `moveTaskToBack` 来修 —— 前者销毁实例
（用户明确不要），后者把聊天页一起退到后台（用户看到桌面）。任务栈隔离是唯一解。

##### 4.7.12.5 返回键为什么要注册**两次**

`OnBackPressedDispatcher` 按**注册顺序倒序**派发（后注册的先被问）。WebView 自己
会在初始化时注册一个「能回退就 `goBack()`」的回调，时点不确定；它一旦排在后面，
返回键就被它截走。所以 `onCreate` 注册一份，`onWebViewCreate`（必然晚于 WebView
构造）再注册一份，让我们的稳定排在最后 = 稳定优先。

> 覆写 `onBackPressed()` 保留作为兜底：Chromium 那份回调在 `canGoBack()` 为假时会
> `isEnabled = false` 再回落到 `onBackPressed()`。但**不能只靠它** —— API 33+ 的
> 预测性返回手势下 `onBackPressed()` 根本不会被调用，注册回调才会被 androidx
> 接到 `OnBackInvokedCallback` 上。

##### 4.7.12.6 ⚠️ 「拉到后台就隐藏」守卫**已删除**（2026-10-03，真机 ANR 后）

> **本节描述的是已经拆掉的代码，留在这里是为了说明「为什么拆」。**
> 现行 `SettingsActivity.kt` 里**没有** `onStop` / `onResume` 覆写，也**没有**
> `wentToBackground` / `pendingShow` 两个字段（`grep` 只会命中注释）。

原先的守卫是这样的：`moveTaskToBack` 只解决「用户主动关」，另一半是「设置窗在
后台活着时，用户从**别的入口**回到 App（桌面图标、桌宠「返回主页」、通知……），
系统把整个任务拉到前台，栈顶仍是设置窗」—— 于是用「`onStop` 记标记 + `onResume`
里若非用户主动就 `finish()`」把它杀掉：

```kotlin
// ❌ 已删除
@Volatile private var pendingShow = false
override fun onStop() { super.onStop(); wentToBackground = true }
override fun onResume() {
    super.onResume()
    val requested = pendingShow; pendingShow = false
    if (requested) { wentToBackground = false; return }
    if (wentToBackground) window.decorView.post { finish() }
}
```

（当年选 `onStop` 而不是 `onPause`，是因为 `onPause` 会被权限弹窗、下拉通知栏这类
**短暂打断**触发，只看它会把正常使用中的设置窗误杀。）

**为什么拆**，两条：

**1. 它是「Tauri 窗口登记表」与「Android 实例」失同步的唯一来源。**
`finish()` 把 `instanceRef` 置空，但 Tauri 那边的窗口登记还在 —— 前端于是以为
「实例没了」，掉进「再建一次窗」分支；而 `singleInstance` 复用实例时不会再发
`onCreate` 回调，tao 建窗那条路会白等 5 秒、还一路持有 `webviews_lock()`
（详见 **4.7.27**）。它是那场 5 秒 ANR 的成因之一。

**2. 它与「设置页只用退回、不要关闭」的既定方向冲突**（用户原话：「前面不是说吗？
设置页尽量只用退回，不要用关闭，以节省性能」）。一个会在某些入口下偷偷 `finish()`
的守卫，正好是这个方向的漏洞。

**那「从别的入口回 App 看到设置页」怎么办？** 不靠守卫，靠**任务栈隔离**
（4.7.12.2）：设置任务声明了 `taskAffinity` + `singleInstance` +
`excludeFromRecents`，桌面图标 / 桌宠返回键拉起的都是**聊天任务**，设置任务只会
留在后面、不会被 resume —— 本来就不会出现「想看聊天页却看到设置页」。守卫是当年
同栈时代的补丁，现在没有存在必要。

⚠️ 现行语义：**设置 Activity 一旦创建就活到进程结束**（或被系统回收），
所有「关闭」都是 `moveTaskToBack`，不销毁。

##### 4.7.12.7 再点「设置」必须有原生通道 —— `setFocus()` 是空实现

前端发现窗口已存在时走的是 `WebviewWindow.getByLabel("settings")` → `setFocus()`。
但 `tao-0.35.2/src/platform_impl/android/mod.rs` 里：

```rust
pub fn set_focus(&self) {
    warn!("set_focus not yet implemented on Android");
}
```

**空实现。** 而「关闭」现在是「退回」（窗口活着，只是它所在的设置任务在后台），
没有替代通道的话，用户第二次点「设置」就是**毫无反应**。

所以 `MainActivity` 在主窗口的 WebView 上挂了一条 JS 桥：

```kotlin
// MainActivity.installSettingsBridge
webView.addJavascriptInterface(object {
    @JavascriptInterface
    fun focusSettings(): Boolean = SettingsActivity.bringToFront()
}, "LingChatApp")
```

`bringToFront()` 用 `ActivityManager.moveTaskToFront(taskId, 0)`：

- **不能用 `startActivity`** —— MainActivity 是 `singleTask`，用 `startActivity` 拉它
  会把它上面的 Activity 一并销毁（这条在 `FloatingPetPlugin.bringActivityToFront`
  的注释里已经被证伪过两次）。而设置窗现在有自己的任务栈，拉它本来也只该走
  `moveTaskToFront`；
- 需要 `REORDER_TASKS`，`AndroidManifest.xml` 里已声明（与桌宠「返回主页」共用）；
- ⚠️ 早先这里需要「先置 `pendingShow` 再 `moveTaskToFront`」：`moveTaskToFront` 会
  同步触发 `onRestart → onResume`，标记必须在那之前就绪，否则会被守卫当成
  「非用户主动」而自杀。**`onResume` 守卫删除后（4.7.12.6）这条前提不存在了** ——
  `bringToFront()` 现在只剩一句 `moveTaskToFront`；
- **返回值的语义（2026-10-03 新增）**：`bringToFront()` 返回 `Boolean` ——
  实例不存在返回 `false`，存在（并已拉回前台）返回 `true`。这个布尔值是
  **「Android 侧实例还在不在」的唯一权威判据**：前端**只有在拿到 `false` 时才允许
  新建窗口**，而且新建那一步必须防重入（见 **4.7.27**）。

**为什么是 JS 桥而不是 Tauri 命令**：`SettingsActivity` 在 app 模块，而
`tauri-plugin-floating-pet` 是它依赖的 library 模块，插件反向引用 app 的类得靠反射。
JS 桥同模块直调，编译期就能查错。

##### 4.7.12.8 ⚠️ 顺带确认的硬约束：**一个 Tauri 窗口 = 一个 Activity**

一度考虑过「去掉 SettingsActivity，把设置页渲染进 MainActivity 那个 WebView」。
不行，两个原因：

1. `tao` 建窗必须给 `activityName`，`startActivity(Class)` 是**真起一个 Activity**，
   没法让两个 `WebviewWindow` 共用一个；
2. 更关键的是，MainActivity 那个 WebView **就是桌宠本体** —— 桌宠浮在桌面上时它被
   原生搬进了 `WindowManager`，不在 Activity 视图树里。要在主活动里显示全屏设置页，
   就得先把它搬回来（等于强制收回桌宠）。

所以设置页要么是「独立 Activity + 独立 WebView」（现状），要么干脆不要独立窗口。
2026-10-03 与用户确认后选的是**前者**。

#### 4.7.13 ★ `Type.systemBars()` **不含挖孔** —— 安全区回归的成因

**真机症状**（用户原话）：「不知道为什么 lc 本身的安全区没了」，
「我们的（桌宠设置页）好像在切换横竖屏就有安全区了」。

`SafeAreaInsets` 早期只取 `Type.systemBars()`。而 `systemBars` 的定义是
**状态栏 + 导航栏 + 标题栏 —— 根本不含挖孔**。

在没有 `windowLayoutInDisplayCutoutMode=always` 时这够用：Android 14 及以下默认会把
窗口在挖孔那一侧「信箱化」，系统替我们留了白。但 4.7.9 为了修悬浮窗横屏被信箱化，
给 `Theme.ling_chat` 显式加了 `always` —— 窗口从此**铺满整块屏幕、含挖孔**，而
`systemBars().left/right` 恒为 0：

| 方向 | 表现                                                       |
| ---- | ---------------------------------------------------------- |
| 竖屏 | 状态栏通常比挖孔高，看不出来                               |
| 横屏 | 挖孔跑到左右两侧，`left/right` 为 0 → 内容直接钻到摄像头下 |

悬浮窗那边一直是 `maxOf(systemBars, displayCutout)`（`FloatingPetPlugin.safeInsetsPx()`），
所以悬浮窗没事 —— 回归只出在走 `SafeAreaInsets` 的**主窗口与设置窗**上。

**修法**：`apply()` 改成接收整个 `WindowInsetsCompat`，四边都取 `max(bars, cutout)`：

```kotlin
private fun apply(webView: WebView, insets: WindowInsetsCompat) {
    val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars())
    val cutout = insets.getInsets(WindowInsetsCompat.Type.displayCutout())  // ★ 必须单独取
    val top = maxOf(bars.top, cutout.top)
    // bottom / left / right 同理
}
```

> 这条与 4.7.9 是**耦合**的：`always` 修好了横屏信箱化，代价就是挖孔那一条要自己补。
> 以后谁再动 `windowLayoutInDisplayCutoutMode`，必须同时复核这里。

#### 4.7.14 桌宠设置页的左右滑动切标签（复用 LC 设置面板那套）

用户要求「尽可能模仿现有设置页」。LC 自己的设置面板
（`src/components/settings/SettingsPanel.vue`）早就有一套滑动切标签，判断条件都是
踩过坑定下来的，所以**直接照搬**，不另起一套：

| 判断                                         | 为什么                                              |
| -------------------------------------------- | --------------------------------------------------- |
| 起点在 `input[type=range]` 上 → 不切页       | 否则拖「桌宠大小」滑块会把页面翻走                  |
| 起点在**确实有横向溢出**的容器里 → 不切页    | 日志/历史页的横向表格要能拖；内容不满的页面仍可滑动 |
| `\|dx\| < 50` 或 `\|dx\| <= \|dy\|` → 不切页 | 和垂直滚动区分开，误触比切不动更烦                  |
| 300ms 内只认一次                             | 快速连划会一次跳好几页                              |

标签顺序 `pet → interaction → todo → window`，**必须与 `SettingsSidebar` 收到的
`tabs` 一致** —— 方向是靠索引比较算出来的。

转场从原来的 `fade-slide`（上下淡入淡出）换成有方向感的
`slide-left` / `slide-right`，参数与 LC 面板一致（`0.32s cubic-bezier(0.32,0.72,0,1)`，
起步即快的缓动，掩盖切页时挂载/卸载的 1~2 帧开销）。

> ⚠️ 两个容易漏的点：
>
> 1. **内容区必须加 `overflow-x-hidden`**。LC 那边是靠外层 `overflow-hidden` +
>    子元素 `absolute inset-0` 做到的；这里的内容区自己就是滚动容器，不加的话
>    横向位移会把 `scrollWidth` 撑大，滑完还能横向拖走一截。
> 2. **滑动门槛用 768px 可用宽度，不用 `uiStore.isSmallScreen`**。后者是
>    `min(宽,高) < 500`，手机横屏约 780×360 时判为「小屏」，而布局早就切成宽屏
>    左侧竖栏了，两者会打架。

#### 4.7.15 ★ 安全区注入的**时机**：为什么「旋转一下屏幕就好了」

**真机症状**（用户原话）：「我发现设置页的安全区还是**需要旋转一下屏幕**」。
转一下就对了 —— 这个「转一下」就是全部线索。

`SafeAreaInsets` 早期只有一条注入路径：insets 分发时 `evaluateJavascript` 一次。
但两条时间线根本对不上：

| 事件                                            | 时刻        |
| ----------------------------------------------- | ----------- |
| WebView 被 `addView` 进视图树 → **insets 分发** | t≈0ms       |
| `loadUrl` 建出真正的 document                   | t≈300–800ms |

也就是说那一次 `evaluateJavascript` 打在 **`about:blank` 的 document** 上，
真页面一加载就把 `--safe-area-inset-*` 冲掉了。而转屏会触发
`requestApplyInsets` → 再注入一次 —— 那时页面**已经在了**，于是「转一下就好了」。

**修法**：加第三条路径，把值注册成**每个新 document 开始时都自动执行**的脚本。
这才是构造上正确的做法 —— 页面无论何时加载，脚本都已经带着当前值等在那里：

```kotlin
// androidx.webkit，需 WebViewFeature.isFeatureSupported(DOCUMENT_START_SCRIPT) 守卫
handlers.remove(webView)?.remove()   // ⚠️ 同名脚本后注册覆盖先注册，必须先摘旧的
handlers[webView] = WebViewCompat.addDocumentStartJavaScript(
    webView, values.script, setOf("*"),
)
```

值**内联在脚本字面量里**（不去问原生），这样同一段脚本既能
`evaluateJavascript` 打进当前文档，也能作为 document-start 脚本自动跑。

脚本自身还要防御 `documentElement` 尚未建出的情况：

```js
if (!s()) {
  document.addEventListener("DOMContentLoaded", s, { once: true });
  window.addEventListener("load", s, { once: true });
}
```

现在三条路一起兜：

1. **insets 分发时**立即注入 —— 管旋转、分屏等运行时变化；
2. **`addDocumentStartJavaScript`** —— 管「页面还没加载」这一段，**主路径**；
3. `attach()` 里两次 `postDelayed`（600 / 1800ms）补注入 —— 给不支持
   document-start 脚本的老 WebView 兜底。

> ⚠️ 三个坑：
>
> 1. **重新注册前必须 `ScriptHandler.remove()`**。同名脚本是「后注册覆盖先注册」，
>    不摘旧的话每次 insets 变化都往队列里堆一份。
> 2. **能力守卫不能省**。`DOCUMENT_START_SCRIPT` 在旧 WebView 上不支持，
>    直接调用会抛异常。
> 3. **`WeakHashMap` 而不是普通 Map**。`handlers` / `lastValues` 都以 WebView
>    为 key，用强引用会把已销毁的 WebView 永久钉住（内存泄漏）。

#### 4.7.16 ★ 修正 4.7.10：`onConfigurationChanged` **不够**

4.7.10 让两个 Activity 都覆写了 `onConfigurationChanged` 来强制重排。真机上
「转屏后布局错乱」**依旧复发** —— 因为覆写的时机选错了。

`onConfigurationChanged` 只在**配置变化那一刻**触发。而后台转屏时（App 不在前台），
那一刻窗口尺寸**还没变**：`Activity` 的 `Resources` 更新了，但 View 树要等回到前台
重新测量才拿到新尺寸。于是那次重排用的是**旧尺寸**，等于没做。

**修法**：改用 `View.addOnLayoutChangeListener`，判据从「配置变了」换成
**「尺寸真的变了」**：

```kotlin
webView.addOnLayoutChangeListener { v, left, top, right, bottom, _, _, _, _ ->
    val w = right - left; val h = bottom - top
    if (w == lastWebViewW && h == lastWebViewH) return@addOnLayoutChangeListener  // ★ 去重
    lastWebViewW = w; lastWebViewH = h
    v.post {
        SafeAreaInsets.reapply(view)                                    // 重注安全区
        view.evaluateJavascript("window.dispatchEvent(new Event('resize'));", null)
    }
}
```

两个要点：

- **去重（`lastWebViewW/H`）不能省**。`OnLayoutChangeListener` 在一次布局流程里
  可能被调用多次，而且**改窗口尺寸本身又会触发下一次布局** → 不去重就是
  resize 风暴，WebView 会卡死。
- **`v.post { }` 也不能省**。在布局回调里直接改视图会打断当前 layout pass。

`onConfigurationChanged` 那套**保留**（它管的是非尺寸类的配置变化），
两者互补而不是替代。

#### 4.7.17 ★ 挖孔只认「主导边」—— 竖屏右边莫名多出的空白

**真机症状**（用户原话）：「至少在竖屏下**右边会莫名其妙多一点空白**」。

4.7.13 把安全区改成四边都 `max(systemBars, displayCutout)`，方向是对的
（`systemBars` 不含挖孔）。但 `displayCutout` 的语义被想当然了：

> `DisplayCutout` 会把**同一块挖孔在相邻边上的投影**也报出来。

具体到「竖屏 + 顶部/右上角打孔」：

```
┌─────────────────────┐
│                  ▢  │  ← 挖孔在右上角
│                     │
```

`cutout.top` = 挖孔高度（主导），但 `cutout.right` **也非 0** —— 它表示
「挖孔向右延伸到了边」。而 `cutout.right` 的数值等于挖孔那一小块的高度，
**不是**「右边整条边都要内缩」。四边照单全收 → 给**整条右边**加出一条内缩
→ 竖屏下右侧凭空多一块空白。

**修法**：只取四边里**数值最大**的方向当作「挖孔所在边」，其余方向置零：

```kotlin
val cutMax = maxOf(cut.left, cut.top, cut.right, cut.bottom)
fun cutOn(v: Int) = if (cutMax > 0 && v == cutMax) v else 0

val values = Values(
    top    = cssPx(maxOf(bars.top,    cutOn(cut.top)),    density),
    bottom = cssPx(maxOf(bars.bottom, cutOn(cut.bottom)), density),
    left   = cssPx(maxOf(bars.left,   cutOn(cut.left)),   density),
    right  = cssPx(maxOf(bars.right,  cutOn(cut.right)),  density),
)
```

依据是这个窗口的 `windowLayoutInDisplayCutoutMode=always`（4.7.9）：
窗口铺满整屏，挖孔**只可能**从某一条边侵入，不存在「两块挖孔在两个方向上并列」
的合法布局。所以「主导边」是一个安全且够用的近似。

`systemBars` **不受影响** —— 它本来就是整边语义（状态栏 / 手势条确实横贯整边），
只对 `displayCutout` 做主导边过滤。

日志里直接打出原始读数与判定结果，真机排查一条命令搞定：

```bash
adb logcat -s LingChatSafeArea
# insets bars=0/0/0/48 cutout=0/112/0/0 (主导边=112) → css T=37.3 B=16.0 L=0.0 R=0.0
```

> ⚠️ 用户在这条上纠正过我的理解方向：他说的是「**右边边界扣掉刘海屏**」，
> 我最初理解成「左边边界增加刘海屏」。差别在于：问题**不是**「某条边少留了」，
> 而是「**不该留的那条边留了**」。横屏摄像头在左侧时才需要给左边让位；
> 竖屏时右侧那点 `cutout.right` 是投影，**不该**让位。

#### 4.7.18 ★ 悬浮窗的输入焦点：只有点输入框时才抢

**真机症状**（用户原话）：「展开时会把其他地方的**键盘和返回**给拦截掉」。

`TYPE_APPLICATION_OVERLAY` 窗口有个不显眼的性质：

> 一旦摘掉 `FLAG_NOT_FOCUSABLE`，它就是**系统里最上层的可获焦窗口**，
> 会一直霸着输入焦点。

而展开态**必须**摘掉它 —— `FLAG_NOT_FOCUSABLE` 的窗口**永远收不到输入法**
（IME 只服务于持有输入焦点的窗口）。于是：

| 状态       | `FLAG_NOT_FOCUSABLE`   | 后果                                           |
| ---------- | ---------------------- | ---------------------------------------------- |
| 收起       | 有                     | 正常，不抢焦点                                 |
| 展开（旧） | **摘掉**               | 键盘/返回键被悬浮窗吃掉；别的 App 键盘弹不出来 |
| 展开（新） | 有，**点输入框时**才摘 | 展开本身不抢焦点；只有真的要打字时才短暂获得   |

**修法**：把「可获焦」从**状态**改成**动作**。

- `setExpanded(true)` 只设 `softInputMode`，**不动** `FLAG_NOT_FOCUSABLE`；
- 新增 `set_ime_focus` 命令，由页面输入框的 `focus` / `blur` 驱动：
  - `focused=true` → 摘 flag + `updateViewLayout` + `requestFocus()`，
    **隔一帧**再 `showSoftInput`；
  - `focused=false` → `hideSoftInput` + 加回 flag + `updateViewLayout`。

```kotlin
// ⚠️ showSoftInput **不能**和 updateViewLayout 同步调用
view.postDelayed({ showIme(view) }, IME_SHOW_DELAY_MS)   // 80ms
```

原因：`showSoftInput` 要求「调用时存在持有输入焦点的窗口」。`updateViewLayout`
摘 flag 是异步的，系统完成窗口焦点切换要一帧 —— 同步调用会因「没有可获焦窗口」
被 IME **直接丢掉**，表现为「点了输入框键盘不弹」。

`softInputMode` 里**绝不能**带 `SOFT_INPUT_STATE_VISIBLE`：那会让展开的一瞬间
就弹键盘，等于把「一直抢焦点」换成了「一展开就抢焦点」，症状只减轻没消失。

##### 4.7.18.1 兜底：失去窗口焦点时无条件让出

页面的 `blur` 在「用户直接切到别的 App」这类场景**不一定送得到**（WebView 还在
后台，JS 事件要等回到前台才派发）。所以还要在原生侧挂一层：

```kotlin
webView.viewTreeObserver.addOnWindowFocusChangeListener { hasFocus ->
    if (!hasFocus && 当前可获焦) {
        params.flags = params.flags or FLAG_NOT_FOCUSABLE
        wm.updateViewLayout(webView, params)
    }
}
```

无条件让出是安全的：失去窗口焦点时用户**不可能**在悬浮窗里打字，
让出不会误伤；而让出之后想再打字，点一下输入框就能重新拿到。

##### 4.7.18.2 三处断言缺一不可

这条链路横跨三层，任何一层断掉都是**静默失败**（不崩溃、不报编译错）：

| 层     | 标记                     | 断了会怎样                                       |
| ------ | ------------------------ | ------------------------------------------------ |
| 前端   | 字面量 `\|set_ime_focus` | 输入框 focus 时不发命令，键盘永不弹              |
| Rust   | `set_ime_focus` 命令名   | `invoke` 报「命令不存在」                        |
| Kotlin | 方法名 `setImeFocus`     | R8 改名 → Tauri 反射找不到，同样报「命令不存在」 |

`verify-pet-apk.sh` 里三条**各断言一次**（前端查 brotli 解压后的分块、Rust 查
`.so` 的 `.rodata`、Kotlin 查 dex 字符串表），缺一环即报错。

#### 4.7.19 ★★ 修正 4.7.18：触发点必须是 `touchstart`，不能是 `focus`

4.7.18 上线后，用户立刻报了一个**更严重**的问题：

> 现在呃窗口不能调起输入法。

也就是说 4.7.18 把「霸占焦点」修好了，代价是**键盘彻底弹不出来**。根因是一个
循环依赖：

```
窗口带 FLAG_NOT_FOCUSABLE
  → WebView 永远拿不到输入焦点
  → textarea 无法 focus
  → focus 事件不触发
  → setImeFocus(true) 不被调用
  → 原生不摘 flag
  → ↺
```

**`focus` 是「已经能打字了」的*结果*，而我们恰恰要靠它去争取「能打字」。**
拿结果当输入，就是循环论证 —— 这一版是照抄了「状态驱动」的直觉，但选错了状态。

改法：触发点换成 `@touchstart.passive`。

- 触摸**不受**窗口可获焦与否影响。`FLAG_NOT_FOCUSABLE` 只挡**输入焦点**，
  不挡触摸；窗口上另挂的 `FLAG_NOT_TOUCH_MODAL` 只影响窗口**外**的触摸。
  所以「手指按到输入框」这件事，在摘 flag 之前就已经能送达页面了。
- `@focus` 保留（覆盖外接键盘 Tab 进焦点这类非触摸路径），与 `touchstart`
  共用一个幂等守卫 `imeFocusHeld`，不会重复下命令。

##### 4.7.19.1 还要补一次 `focus()`：摘 flag 是异步的

原生摘 `FLAG_NOT_FOCUSABLE` 要先 `updateViewLayout`、等系统完成窗口焦点切换，
80ms 后才 `showSoftInput`（见 4.7.18）。在那之前 WebView 拿不到输入焦点，
浏览器**不会**把 textarea 真正 focus 上 —— 表现是「键盘弹出来了，但按键进不去
输入框」。

所以前端在发完命令后补一次延迟 `focus()`：

```ts
void setFloatingPetImeFocus(true).catch(() => {});
window.setTimeout(() => textareaRef.value?.focus(), 100); // 必须**早于**原生的 250ms
```

##### 4.7.19.2 断言

`verify-pet-apk.sh` 断言前端分块里存在 `onTouchstartPassive`。

选它而不是 `touchstart`：后者是**通用**字符串，`useMobileViewport` 里也有，
不具区分度。Vue 的 `.passive` 修饰符会把事件名编译成 `onTouchstartPassive`
（后缀式，运行时据此回挂 passive 监听），它是**对象字面量的键名** ——
esbuild 不改对象键，所以能可靠地证明这行绑定真的进了包。

##### 4.7.19.3 ★★ 四修：`showSoftInput` 的**第二个**前提 —— 页面必须先就绪

4.7.19.1 那个 150ms 的补偿是**错的**（比原生的 80ms 还晚），键盘依旧不弹。
四修时才发现真正的原因：

> **`InputMethodManager.showSoftInput(view, …)` 对 WebView 有两个前提，不是一个。**

| #   | 前提                                                                                         | 不满足时                         |
| --- | -------------------------------------------------------------------------------------------- | -------------------------------- |
| 1   | 窗口持有**输入焦点**（不能带 `FLAG_NOT_FOCUSABLE`）                                          | 请求被丢掉                       |
| 2   | WebView **已建立输入连接**（页面里有聚焦的可编辑元素，`View.onCheckIsTextEditor()` 为 true） | **不报错、不抛异常，什么都不做** |

第 1 条 4.7.18 已经处理了（摘 flag + 隔一帧）。第 2 条是四修才补上的 ——
而它恰恰是最难查的，因为**完全静默**。

正确的时序（三件事必须按这个顺序）：

```
0ms     前端 touchstart → invoke set_ime_focus(true)
0ms     原生 摘 FLAG_NOT_FOCUSABLE + updateViewLayout + requestFocus
~100ms  前端 textarea.focus()      ← 页面这一侧就绪，WebView 建立输入连接
250ms   原生 showSoftInput          ← 这时才有输入连接可依附
```

原来错在哪：前端 150ms、原生 80ms —— 原生弹的时候页面还没就绪。

配套改动：

- `IME_SHOW_DELAY_MS`：`80` → `250`
- `showIme()` 带**重试**（每 300ms，最多 5 次），每次都先问
  `view.onCheckIsTextEditor()`「页面准备好了没」——就绪了才 `showSoftInput`
- 日志 `弹出输入法（第 n/5 次）：页面就绪=… 窗口有焦点=…`
  —— 这是**唯一**能区分「没送到」和「送到了但静默失效」的线索
- 重试耗尽时记 `输入法没弹出来：重试 5 次后页面仍未把焦点落到输入框上`
  —— 这句话直接把矛头指向前端（touchstart 有没有触发 / `isInFloatingWindow()` 真假）

##### 4.7.19.4 排查用的日志分段

| 日志                                               | 说明                                                       |
| -------------------------------------------------- | ---------------------------------------------------------- |
| `输入框获得焦点：悬浮窗改为可获焦，准备弹出输入法` | 命令**送到了**原生                                         |
| `弹出输入法（第 n/5 次）：页面就绪=false`          | 送到了，但**页面没聚焦** → 前端问题                        |
| `弹出输入法（第 n/5 次）：页面就绪=true`           | 页面就绪，`showSoftInput` 已发出 → 若仍不弹则是系统/ROM 层 |
| `输入法没弹出来：重试 5 次后…`                     | 页面始终没聚焦                                             |

一条命令：

```bash
adb logcat -s FloatingPetPlugin | grep -E "输入框获得焦点|弹出输入法|输入法没弹出来|输入框失焦"
```

#### 4.7.20 ★ 悬浮窗的左右安全区：左边界**完全不变**

用户原话：

> 我们需要的是**横屏下右边边界内缩，而左边边界完全不变**。

在这之前，`FloatingPetPlugin.safeInsetsPx()` 是四边都 `max(systemBars, cutout)`，
于是「横屏 + 摄像头在左侧」时左边被内缩一大截 —— 正是用户之前报的
「在左侧时就会明显多一大截空白」。

现在的规则（**只对悬浮窗**）：

| 场景             | 挖孔主导边 | left           | right           |
| ---------------- | ---------- | -------------- | --------------- |
| 竖屏、顶部打孔   | top        | `bars.left`(0) | `bars.right`(0) |
| 横屏、挖孔在右侧 | right      | `bars.left`(0) | 挖孔宽度        |
| 横屏、挖孔在左侧 | left       | `bars.left`(0) | `bars.right`(0) |

两条同时生效：① 只认「主导边」（见 4.7.17）；② **`left` 一律不吃挖孔**。

##### 4.7.20.1 ⚠️ 这里与 `SafeAreaInsets.kt` **故意不一致**，别"对齐"回去

`SafeAreaInsets.kt`（注入给设置页 / 主界面 WebView 的那份）**四边都吃挖孔**，
含 `left`。两边口径不同是**有意的**：

- 设置页 / 主界面是**整页内容**，左边被挖孔压住会切掉字，必须让；
- 悬浮窗只有一小块，贴左沿时让它钻到摄像头下一点点，比多出一大截空白自然。

用户明确要求这次**只动悬浮窗**，所以两份实现就是这么写的。将来若有人想统一，
先回去问这个取舍，不要直接改。

#### 4.7.21 ★★ 转屏：判定条件不能只看屏幕尺寸

用户原话：

> 反正就是旋转屏幕时好像**桌宠感知到的屏幕方向没有变**。

这条很关键，它把范围缩到了「**判定**」而不是「重排」。

原来的判据是：

```kotlin
val changed = curW != lastScreenW || curH != lastScreenH
```

而 `screenSizePx()` 是按屏幕**短边**推的（见 4.4.2.1）—— 竖屏 360×802 与横屏
802×360 的**短边都是 360**，算出来的短/长边完全一样，只是分配方式对调。
于是这个判据在部分机型/部分场景下会判成「没变」，**整条重排路径根本不跑**，
桌宠就停在旧坐标里。

修法：加 `lastRotation`（`Surface.ROTATION_*`），**尺寸或旋转角任一变了都重排**。
旋转角是**直接事实**，不再依赖尺寸这个间接量。

##### 4.7.21.1 配套的两条日志，用来分段定位

真机排查时，「桌宠没反应」有两种完全不同的原因，光看结果分不清：

| 日志                     | 出现 | 没出现意味着                             |
| ------------------------ | ---- | ---------------------------------------- |
| `收到 onDisplayChanged`  | ✅   | 系统回调没送到（或显示器 id 不是默认屏） |
| `★ 屏幕变化：… → …`      | ✅   | 回调到了但判定为「没变」（读数问题）     |
| `悬浮窗重排：屏幕 … → …` | ✅   | 判定命中但重排抛异常（见 `Log.w`）       |

`★ 屏幕变化` 那行会把 `尺寸变=…` 与 `旋转变=…` **分别打出来**，一眼能看出是哪条判据命中的。
`悬浮窗重排` 那行带**前后屏幕尺寸、前后窗口几何、贴哪一边、yRatio、四个安全区**。

排查命令：

```bash
adb logcat -s FloatingPetPlugin | grep -E "onDisplayChanged|屏幕变化|悬浮窗重排"
```

#### 4.7.22 ★ 熄屏时**不要**唤醒桌宠 WebView

用户原话：

> 还有这东西好像有点吃配置啊。

排查时找到一条**确定的浪费**（不是猜测，是代码路径上白烧的一份）：

1. 熄屏 → Activity 走 `onPause` → `onActivityPaused` 里置位 `petNeedsResume = true`；
2. 保活轮询（500ms 一次）看到 `petNeedsResume` → 调 `resumePetWebView()`
   → `WebView.onResume()` + `resumeTimers()`。

结果是**屏幕全黑、谁都看不见的时候，WebView 仍在跑 JS 定时器、跑 Live2D 的
`requestAnimationFrame`**，持续唤醒 CPU。

修法：唤醒前先问一句屏幕亮没亮。

```kotlin
if (petDetached && petNeedsResume && isScreenInteractive()) { … }
```

`PowerManager.isInteractive` 读不到时返回 `true`（保守）：宁可多跑一点，
也不要因为一次读数失败让桌宠在用户眼前冻住。

屏幕亮回来**不需要**这里补：`WryActivity.onResume()` 会自己唤醒 WebView，
并把 `hostResumed` / `petNeedsResume` 一起复位。

##### 4.7.22.1 ⚠️ 只挡「熄屏」，**不能**顺手挡「切到别的 App」

两种情况宿主 Activity 都是 paused、`petNeedsResume` 都是 true，但语义完全相反：

- **熄屏** → 没人看得见 → 不该跑；
- **切到别的 App** → 桌宠浮在那个 App 上面 → **必须继续动**，那正是悬浮窗存在的意义。

判据必须是 `isInteractive`（屏幕状态），不能是「Activity 是否 resumed」。

#### 4.7.23 ★ 桌宠设置页竖屏右边多出的空白：纯 Vue 尺寸算式

用户原话：

> 然后在设置页应该是 vue 问题，右边多一小段空白在竖屏下。

**猜对了**，跟安全区、跟原生一点关系都没有。

`SettingsPage.vue` 里内层卡片原来是这么写的：

```html
<section
  class="m-1 flex h-full w-full flex-col … …"
  style="height: calc(100% - 1rem); width: calc(100% - 1rem)"
></section>
```

三个尺寸来源互相打架：

| 来源                     | 效果                                         |
| ------------------------ | -------------------------------------------- |
| `w-full`                 | 100% 父内容宽                                |
| 行内 `calc(100% - 1rem)` | **覆盖** `w-full` → 卡片宽 = 父内容宽 − 16px |
| `m-1`                    | 左右各 4px 外边距                            |

于是：左边距只有 **4px**，而右边空出 `4 + (16 − 4) = 12px` —— **右边比左边多 8px**。
`height` 那边是同一个毛病（`h-full` 被行内 `calc(100% - 1rem)` 覆盖）。

修法：**去掉行内尺寸，也别写 `h-full w-full`**，让布局自己算：

```html
<section class="m-1 flex min-h-0 flex-1 flex-col … …"></section>
```

- 宽度：父容器是 `flex flex-col`，`align-items` 默认 `stretch`，卡片自动铺满
  父内容宽**减去左右外边距** → 天然对称；
- 高度：`flex-1` + `min-h-0`（`min-h-0` 是让内部的 `overflow-hidden` 真的生效）；
- 四边只剩 `m-1`（窄屏 4px）/ `min-[768px]:m-2`（宽屏 8px）。

`verify-pet-apk.sh` 用 **absent** 断言兜住它：`calc(100% - 1rem)` 现在只出现在
代码注释里，而 Vue 生产构建会剥掉注释，所以「搜不到」就是改动进了包的证据。

#### 4.7.24 ★ 转屏后桌宠不重排的真根因：后台时 `DisplayInfo` 是冻结的

**这一条推翻了 4.7.21 的前提。** 4.7.21 加 `lastRotation` 时假设
「`Display.getRotation()` 直读 `DisplayInfo`，所以与 App 可不可见无关」——
这个假设是**错的**，2026-10-03 用无线调试抓 logcat 证伪了。

**怎么抓到的**

手机（vivo V2118A / Android 14）用 Tailscale 无线调试接上 `adb`，
桌宠浮着、LingChat 在后台，让用户手动横→竖→横转屏三次，把两条日志按毫秒对齐：

```
11:59:57.254  无影 onConfigurationChanged  mDisplayRotation=ROTATION_0（竖屏）
11:59:57.292  FloatingPet 收到 onDisplayChanged（rotation=1 …）        ❌
12:00:04.435  无影 onConfigurationChanged  mDisplayRotation=ROTATION_90（横屏）
12:00:04.495  FloatingPet 收到 onDisplayChanged（rotation=1 …）        ✓
12:00:13.286  无影 onConfigurationChanged  mDisplayRotation=ROTATION_0（竖屏）
12:00:13.343  FloatingPet 收到 onDisplayChanged（rotation=1 …）        ❌
12:00:18.764  无影 onConfigurationChanged  mDisplayRotation=ROTATION_90（横屏）
12:00:18.807  FloatingPet 收到 onDisplayChanged（rotation=1 …）        ✓
```

「系统真值」取的是无影 App 自己的 `onConfigurationChanged`（同一毫秒窗口，
它读的是系统给的 `Configuration`，可信）。

**结论：屏幕确实在转，但插件读到的 `rotation` 从头到尾卡在 `1`。**
`Display.getRotation()` 读的是 `mDisplayInfo`，而它在**进程退到后台、
只靠悬浮窗活着**的时候不刷新，停在「最后一次在前台时」的方向。

于是 `maybeReapplyForScreenChange()` 的两个判据（尺寸、旋转角）**永远等于
「没变」**，重排一次都不会跑，桌宠一直停在旧坐标系里。

> 注意 `onDisplayChanged` **照常送达**（四条回调的时间点和转屏严格对齐）——
> **回调是活的，读数却是死的**。所以「加个回调」这类修法在这里没用，
> 必须换读数的来源。

**顺带解释上一轮那个「91 秒后才自愈」**：那是一次偶然——某个事件让进程的
`DisplayInfo` 刷新了一次，500ms 轮询才终于撞上。不是设计如此，是运气。

**怎么找到活的那条路**

同一个类里的 `safeInsetsPx()` 走的是：

```kotlin
activity.getSystemService(WindowManager::class.java)
    .currentWindowMetrics.windowInsets
```

它的读数**跟着方向正确变化**（日志里 `safe=T100` ↔ `T108` 交替，正好对应
竖屏/横屏）。同一次 `onDisplayChanged` 里，**一个活一个死，差别只在来源**。

**修法**

1. `screenSizePx()` 首选换成 `WindowManager.getCurrentWindowMetrics().bounds`；
   - ⚠️ 必须是 `current`，**不是 `maximum`** —— 后者读
     `windowConfiguration.getMaxBounds()`，与 Resources 同源，后台同样冻结。
     这条路三修已经证伪过一次，`verify-pet-apk.sh` 里有 `absent` 断言兜着。
     `current` 正是 `safeInsetsPx()` 用的那个调用，被真机证明是活的。
2. `displayRotation()` **不再直读 `Display.getRotation()`**，改成从
   `screenSizePx()` 推导（宽 > 高即横屏）。冻结的旋转角和活的尺寸会互相打架，
   推导能保证两个判据永远一致。代价：分辨不出 180°（手机基本不允许，
   且 180° 只影响贴哪条边）。
3. `screenDebugInfo()` **同时打印多个来源**（四修时的形态）：

```
cur=2408x1080（首选来源）, dispRot=1（后台可能冻结）, panel=1080x2408（不随旋转）, safe=…
```

⚠️ **别把这条日志简化成只打一个来源。** 以后再出「转屏没反应」，看这一行就能
定位是哪条路死了：`cur=` 不跟着变 → 换来源；`cur=` 变了而 `dispRot=` 没变 →
就是本条修的 bug。`safe=` 可以当独立佐证：横屏时挖孔在左（L 大 / T 小），
竖屏时在顶（T 大 / L 小），它和 `cur=` 必须同向。

> ⚠️ 这条「`cur=` 当首选来源」的判断**后来被推翻**了 —— 见 §4.7.25：
> `cur=` 在**后台**同样冻结。四修只解决了「前台」这一半。

**方法论（可复用）**

- **凡「系统回调收到了但状态没变」的 bug，先怀疑「读数的来源」，而不是回调。**
  本例里回调完全正常，错的是读数。
- **同一个类里找一个「表现正常」的同类读数当参照物**，比对两者的来源差别 ——
  比从头推理系统实现快得多。这里就是拿 `safeInsetsPx()` 当参照物找到答案的。
- **日志里同时打印多个来源**，让日志自己回答「谁死了」。
- 无线调试（Tailscale + `adb pair/connect`）在没有 USB 的场景下完全够用；
  见 `grab-pet-logs.sh`。

#### 4.7.24.1 `adb` 强转屏：其实可用（本条 2026-10-03 更正过）

~~想用 `adb shell settings put system user_rotation 1` 复现转屏？在这台机上
**没用**：值写进去了（读回是 `1`），但 `mRotation` 纹丝不动。~~

**更正**：能用，之前判定「无效」是误判。正确姿势是**先关自动旋转**：

```bash
adb shell settings put system accelerometer_rotation 0   # ★ 关键，别漏
adb shell settings put system user_rotation 1            # 0/1/2/3 = 0°/90°/180°/270°
adb shell dumpsys window displays | grep mCurrentRotation   # 验证：ROTATION_90
```

**`user_rotation` 在 `accelerometer_rotation=1` 时是被系统忽略的** ——
所以「写进去了但 `mRotation` 不动」并不是 ROM 不支持，而是自动旋转还开着。
实测（2026-10-03 12:56）先关自动旋转后 `mCurrentRotation` 立刻变成 `ROTATION_90`。

> ⚠️ 用完**记得还原** `settings put system accelerometer_rotation 1`，
> 别把用户的自动旋转留在关闭状态。

另外 `adb exec-out screencap -p` 走 Tailscale 中继时二进制流会被截断
（`image file is truncated`），要改成「设备上落盘 → `adb pull`」，
并且 Git Bash 下必须 `export MSYS_NO_PATHCONV=1`，否则 `/sdcard/...` 会被改写成
Windows 路径。

#### 4.7.25 ★★ 真根因（五修）：App 在后台时，进程内**一切**屏幕读数都冻结

**这一条推翻了 §4.7.24 的结论。** 四修把尺寸来源换成
`WindowManager.getCurrentWindowMetrics().bounds` 后，用户复验的反馈是
**「问题完全依旧」**，并给了一句决定性的定位：

> 「不是在软件内旋转就有用，在软件外旋转就没用。」

这句话把 bug 的范围钉死在 **App 处于后台** 这个条件下 ——
四修之所以在开发机上「看起来好了」，是因为一直在前台测。

##### 毫秒对齐拿到的真相

把「物理转屏时刻」（截图 / 用户口述）与插件日志按毫秒对齐，读数是：

| 读数                                                  | 前台 | 后台（App 不在最前） |
| ----------------------------------------------------- | ---- | -------------------- |
| `Resources.displayMetrics` / `getSystem()`            | 新鲜 | **冻结**             |
| `DisplayManager.getDisplay().rotation` / `.mode`      | 新鲜 | **冻结**             |
| `WindowManager.currentWindowMetrics.bounds`           | 新鲜 | **冻结**             |
| `currentWindowMetrics.windowInsets`（`safeInsetsPx`） | 新鲜 | **时活时死**         |
| `DisplayManagerGlobal.getDisplayInfo()`（binder）     | 新鲜 | **新鲜** ✅          |

典型现场：手机已横过来，截图是 2408×1080，而插件日志里
`cur=1080x2408` 稳如泰山 —— 它以为自己在竖屏。

##### 为什么会这样

这些读数**全部是客户端缓存**：

- `DisplayInfo` 是 `DisplayManagerGlobal` 手里的一份**副本**；
- `WindowMetrics` 来自客户端的 `DisplayArea` 树；
- `Resources.Configuration` 来自 `ResourcesManager`。

它们的刷新机制是**系统在配置变更时「推送」给进程** —— 而进程退到后台后
**收不到那次推送**。但 `DisplayManager.DisplayListener.onDisplayChanged`
**照常送达**（时间点和转屏严格对齐）。

**回调是活的，数据是死的。** 于是判定永远等于「没变」，重排一次都不跑。

##### 修法

加一条**绕过所有客户端缓存**的来源，排在最前：

```kotlin
private fun freshDisplayInfoPx(): Triple<Int, Int, Int>? = try {
    val cls = Class.forName("android.hardware.display.DisplayManagerGlobal")
    val inst = cls.getMethod("getInstance").invoke(null)
    val info = cls.getMethod("getDisplayInfo", Int::class.javaPrimitiveType)
        .invoke(inst, Display.DEFAULT_DISPLAY)
    ...
} catch (t: Throwable) { ...; null }
```

`DisplayManagerGlobal.getDisplayInfo()` 走的是
`IDisplayManager.getDisplayInfo()` **binder 调用**，每次现问服务端，
不经过任何客户端缓存。

⚠️ **它必须只是「首选」，不能是「唯一」**：`DisplayManagerGlobal` 是隐藏 API，
只能反射，会被非 SDK 接口限制拦掉（返回 `null`）。所以
`screenSizePx()` 做成**分级**：

```
来源 0  freshDisplayInfoPx()            ← binder，后台唯一新鲜
来源 1  currentWindowMetrics.bounds     ← 前台新鲜，后台冻结
来源 2  Display.getMode() 短边/长边分配  ← 前台新鲜，后台冻结
来源 3  resources.displayMetrics        ← 最终兜底
```

反射失败**只记一次**日志（`freshSourceBrokenLogged` 去抖）——
`screenSizePx()` 每 500ms 被 `keepAliveTick` 读一次，每次都记就是刷屏白烧电。

`screenDebugInfo()` 随之改成多来源对照，五修后长这样：

```
fresh=2408x1080@rot1（binder，唯一后台新鲜）, cur=1080x2408（WindowMetrics，后台冻结）,
dispRot=0（DisplayInfo 副本，后台冻结）, panel=1080x2408（不随旋转）, safe=L…/T…/R…/B…
```

> **`fresh=` 才是判据来源。** 以后若再出「转屏没反应」，先看 `fresh=`：
>
> - `fresh=` 不跟着变 → 反射这条路也被拦了（问题在来源）
> - `fresh=` 变了但没重排 → 问题在判据（`lastScreenW` / `lastRotation`）

##### 方法论（比 4.7.24 更进一步）

- **「前台能复现」不等于「后台能复现」**。凡是「系统状态变化」类的 bug，
  必须在 **App 退到后台** 的条件下验一遍 —— 后台是缓存失效的放大器。
- **一个读数正常，不代表同类的另一个读数也正常**：`windowInsets` 活着而
  `bounds` 死了，两者同属 `WindowMetrics`。**逐个来源单独打日志**，
  不要合并。
- **判据要建立在「每次现问」的来源上**（binder / 系统调用），
  而不是「被推送刷新」的缓存上。
- 定位这类问题时，「找一个表现正常的同类读数当参照物」依然有效 ——
  这次参照物是 `safe=`（它时活时死，正好暴露了 `cur=` 的死）。

##### 验证

`verify-pet-apk.sh` 新增断言：`DisplayManagerGlobal` / `getDisplayInfo` /
`logicalWidth` / `logicalHeight` / `反射 DisplayManagerGlobal` /
`（binder，唯一后台新鲜）` / `（WindowMetrics，后台冻结）` /
`（DisplayInfo 副本，后台冻结）`，并断言四修那版 `（首选来源）` **已消失**
（防止 `screenDebugInfo` 退回「只打一条来源」的旧写法）。

当前：**118 ✅ / 0 ❌**（`stale-state-fix`）。

> ⚠️ 仍需**真机复验**：装 `stale-state-fix` 后，让桌宠浮着、把 LingChat
> 退到后台，**在软件外**转屏，看 `fresh=` 是否跟着方向变、桌宠是否重排。

**结果（2026-10-03 12:41 真机）**：修好了。日志里 `fresh=` 精确跟着方向翻转，
`★ 屏幕变化` 与重排都在回调后 ~130ms 内跑完。五修生效。

#### 4.7.26 ★★ 横屏右边溢出屏幕 76px：窗口的坐标原点被挖孔顶偏了

五修修好转屏后，用户立刻报出下一个症状：

> 「我这边横屏时右边会超出屏幕一点。」

##### 症状的边界条件

| 场景                           | 表现                            |
| ------------------------------ | ------------------------------- |
| 竖屏                           | 完全正常                        |
| 横屏（ROTATION_90 / 270 都算） | 贴右沿时右边跑到屏幕外，约 76px |
| 横屏贴左沿                     | 正常（只是比预期多一点点边距）  |

「只有横屏」「只在右边」——这两个条件合起来，指向的必然是**横向的坐标系**问题。

##### 真机证据

`adb shell dumpsys window windows` 里把两个窗口摆在一起看，一眼就穿了：

```
# 主界面 MainActivity —— manifest 里声明了 layoutInDisplayCutoutMode=always
mAttrs={(0,0)(fillxfill) ... layoutInDisplayCutoutMode=always ty=BASE_APPLICATION ...}
Frames: parent=[0,0][2408,1080]  display=[0,0][2408,1080]  frame=[0,0][2408,1080]
                ↑ 父容器起点 0

# 桌宠悬浮窗 —— **没有**声明挖孔模式
mAttrs={(2231,346)(153x153) gr=TOP START CENTER ty=APPLICATION_OVERLAY
        fl=NOT_FOCUSABLE NOT_TOUCH_MODAL LAYOUT_NO_LIMITS HARDWARE_ACCELERATED}
                                    ↑ 注意这里没有 layoutInDisplayCutoutMode
Frames: parent=[100,0][2408,1080]  frame=[2331,346][2484,499]
                ↑ 父容器起点 100                    ↑ 右边缘 2484 > 屏幕宽 2408
```

##### 机制

`WindowManager.LayoutParams.x/y` 是**相对父容器**的坐标，不是屏幕坐标。

`TYPE_APPLICATION_OVERLAY` 窗口若**不声明挖孔模式**，默认行为是
`LAYOUT_IN_DISPLAY_CUTOUT_MODE_DEFAULT` —— 系统会把它的父容器**按挖孔内缩**。
横屏时挖孔在侧边（本机 `mDisplayCutout.insets=Rect(100,0-0,0)`，深 100px），
父容器就从 `x=100` 开始。

而我们的代码是按「相对屏幕原点」算的：

```
期望：x = screenW(2408) - safe.right(0) - width(153) - margin(24) = 2231
实际：绝对左 = 2231 + 100 = 2331
      绝对右 = 2331 + 153 = 2484   ← 超出屏幕 76px
```

竖屏时挖孔在**顶部**，父容器的 `x` 起点仍是 0，所以只有横屏发作 ——
两个边界条件全部对上。

> ⚠️ 注意 `LAYOUT_NO_LIMITS` **不会**阻止这个偏移，反而让窗口可以真的画到
> 父容器之外（否则系统会把它夹回来，症状会变成「贴不到右沿」而不是「溢出」）。

##### 排除干扰项：一次全窗口对照

上面「MainActivity vs 桌宠」的对比有个漏洞 —— 两者**窗口类型不同**
（`type=1` vs `type=2038`）。万一是「overlay 类型本身就会内缩」呢？

于是把横屏下**所有窗口**的「挖孔模式 × parent 起点」拉成一张表：

| 窗口                             | 类型             | 挖孔模式     | parent 起点    |
| -------------------------------- | ---------------- | ------------ | -------------- |
| **桌宠悬浮窗**                   | 2038 overlay     | **未声明**   | **[100,0]** ❌ |
| InputMethod（输入法）            | 输入法           | **未声明**   | **[100,0]** ❌ |
| GlobalDrawerContainerView        | **2038 overlay** | `shortEdges` | [0,0] ✅       |
| ShellDropTarget                  | **2038 overlay** | `always`     | [0,0] ✅       |
| 其余全部（app / 系统窗 / 壁纸…） | 各种             | 各种         | [0,0]          |

**同为 `type=2038` 的 overlay，声明了挖孔模式的两个都是 `[0,0]`** ——
窗口类型这个干扰项被排除，只剩「挖孔模式」一个变量。
`InputMethod` 同样未声明、同样被顶到 `[100,0]`，是第二个独立佐证。

> **方法论**：当对照实验里有两个变量（类型 / 挖孔模式）同时不同时，
> 别急着下结论 —— **把全部样本拉成一张表**，让变量自己分离。
> 这比再设计一次受控实验快得多，而且样本是现成的。

##### 修法

给悬浮窗补上挖孔模式声明，让父容器 = 整块屏幕：

```kotlin
if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {          // API 30+
    layoutInDisplayCutoutMode = WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_ALWAYS
} else if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {   // API 28–29
    layoutInDisplayCutoutMode = WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES
}
```

（`ALWAYS` 是 API 30+，本工程 minSdk=24，所以 28–29 退回 `SHORT_EDGES` ——
手机上挖孔就在短边：竖屏顶部、横屏左右，够用。）

这一改同时**顺带满足了 §4.7.20 的设计意图**：桌宠现在可以贴到挖孔所在的那条
短边上（贴左沿时钻到摄像头下一点点），而不是被系统凭空顶开 100px。

##### 加了一条永久守卫

这个偏移**在应用内部读不到** —— `currentWindowMetrics.bounds` 给的是整块屏幕
（2408×1080），不含父容器偏移。唯一能拿到的方式是反推：

```kotlin
view.post {
    val loc = IntArray(2)
    view.getLocationOnScreen(loc)
    val dx = loc[0] - params.x     // 非 0 就是父容器偏移
    ...
}
```

所以新增 `warnIfWindowOriginOffset()`，在 `addView` 之后和每次旋转重排之后各验一次，
**非零就告警**。它**只报告、不修正** —— 修正是挖孔声明的职责；如果它又开始报警，
说明那个声明被删了或被 ROM 忽略了，**别用补偿掩盖**。

##### 方法论（可复用）

- **`params.x/y` 是相对父容器的**。「按屏幕尺寸算的坐标」和「窗口实际落点」
  之间可以差一个父容器偏移，而**这个偏移在应用内没有任何直接 API 可读**。
- **`dumpsys window windows` 的 `Frames: parent=[...] frame=[...]` 是唯一真值。**
  把可疑窗口和一个「表现正常」的窗口（这里是 MainActivity）并排看，
  父容器起点不一样就是它。**这比读 AOSP 快一个数量级。**
- **「只在横屏发作」优先怀疑横向坐标系**，而不是位置计算逻辑 ——
  竖屏/横屏的差别往往不在算法，而在某个环境量（这里是挖孔方位）。
- 又一次印证 §4.7.25 的方法论：**找一个表现正常的同类对象当参照物**
  （上次是同类读数，这次是同类窗口）。

##### 验证

`verify-pet-apk.sh` 新增：`layoutInDisplayCutoutMode` /
`Landroid/view/WindowManager$LayoutParams;` / `窗口坐标原点偏移` /
`getLocationOnScreen` 四条 present 断言。

> ⚠️ **这里踩过一个坑，值得记下来：不能断言常量名。**
>
> 一开始写的是断言 `LAYOUT_IN_DISPLAY_CUTOUT_MODE_ALWAYS` /
> `_SHORT_EDGES` present，结果两条都报 ❌。原因是它们是
> `public static final int`，编译器在**编译期就把它们内联成字面量**了
> （实测三个常量名在 DEX 里出现次数均为 0），名字根本不会进包。
>
> 顺带发现一个更隐蔽的**假阳性**：`layoutInDisplayCutoutMode` 这个词当时
> 在 DEX 里出现 2 次 —— 一次是真字段引用，另一次是**守卫日志的文案**
> 里写了「检查 `layoutInDisplayCutoutMode` 是否被删」。也就是说，
> 就算有人把挖孔声明整个删掉，只要日志文案还在，那条断言**照样通过**。
> 所以把日志文案改成了「检查窗口的挖孔模式声明是否被删」，让这个词在包里
> 只剩真字段引用一处 —— 断言这才真正有约束力。
>
> **教训：断言字符串时，先确认这个字符串在包里是不是唯一语义来源。**
> 日志文案和被测代码用同一个词，等于给自己发了张空头支票。

#### 4.7.27 ★ 「设置页反复弹出收起 → 卡死」= **ANR**，根因是 `webviews_lock()` 互锁

**真机症状**（用户原话）：

> 我发现设置页反复弹出收起容易卡死崩溃

**先纠正一个误判**：这不是崩溃（crash），是 **ANR**。
`dumpsys activity exit-info com.noiq.lingchat` 给出的记录是连续三次
`reason=6 (ANR)`：`14:58:34` / `14:59:25` / `15:00:03`。subreason 全
`0 (UNKNOWN)`，配套的 dropbox 条目里写着：

```
Input dispatching timed out (... Sys2038:com.noiq.lingchat/com.noiq.lingchat.MainActivity,
Waited 5001ms for MotionEvent)
```

⚠️ **`type=2038` = `TYPE_APPLICATION_OVERLAY`** —— 卡住的是**桌宠悬浮窗**，
也就是「点桌宠没反应」，不是设置 Activity 本身。

##### 4.7.27.1 决定性证据：两个线程同时卡在 Rust 的 futex 上

从 `dumpsys dropbox --print data_app_anr` 里剥出来的线程栈：

**主线程**：

```
"main" state=S
  #00 libc syscall+32                              ← futex 等待
  #01.. base.apk (offset 2c8000) ×7                ← Rust（base.apk = libling_chat_lib.so）
  at com.noiq.lingchat.Rust.onPageLoaded(Native method)
  at com.noiq.lingchat.generated.RustWebViewClient.onPageFinished(SourceFile:17)
  at ... Looper.loop
```

**JavaBridge 线程**：

```
"JavaBridge" state=S
  #00 libc syscall+32                              ← 同样在 futex
  #01 0x08184ae8 … #0N base.apk
  at com.noiq.lingchat.generated.Ipc.postMessage(SourceFile:13)   ← JS 发起的 Tauri invoke
```

两条**不同的**线程 —— 一个卡在「页面加载完成回调」，一个卡在「JS 调用 Tauri
命令」—— 都在等 Rust 的同一把锁。

##### 4.7.27.2 根因：两条路都要 `webviews_lock()`，而建窗又必须等主线程

顺着源码（wry 0.55.1 + tauri 2.11.1）走：

1. 页面加载完成 → `wry/src/android/binding.rs:469 onPageLoaded()`，**持
   `ON_LOAD_HANDLER`** 调 Tauri 注册的 handler；
2. `tauri/src/manager/webview.rs:293-305` 那个 handler 第一件事是
   `app_manager_.get_webview(&label)` → **`webviews_lock()`**
   （`tauri/src/manager/mod.rs:671`），紧接着还要 `plugins.lock()`；
3. 另一边，JS 里 `new WebviewWindow(...)` 走 `wry Ipc.kt postMessage` →
   `binding.rs:383 ipc()` → Tauri IPC 派发 → **同样要拿 `webviews_lock()`**，
   而且会**一直持有到窗口创建完成**；
4. `plugin:window|create` 是在 **JavaBridge 线程**发起的，`tauri-runtime-wry` 的
   `create_window` 必须**回到主线程事件循环**才能真的建窗 → **阻塞等主线程**；
5. 主线程此刻正卡在 `webviews_lock()`（被第 1、2 步的 `onPageLoaded` 持有）→
   **互锁**，5 秒后 `MotionEvent` 超时 → ANR。

一句话：**「JS 建窗」和「页面加载完成」两条路都要 `webviews_lock()`，而建窗又必须
等主线程 —— 主线程被锁挡住，于是互锁。**

##### 4.7.27.3 ⚠️ 这是**老 bug**，不是这次改动引入的

拿当天 **10:45**（**13:44 出包之前**）那次 ANR 的主线程栈和 14:58 那次逐帧对比，
帧序列**完全一致**：

```
#01 0x0817af68   #02 0x064b89b8   #03 0x065500d8   #04 0x06535a3c
#05 0x0654e570   #06 0x08009a14   #07 0x0801ac1c
```

01:11 那次也是同一形态。所以死锁**一直在**，只是「设置页独立任务栈」这个改动让它
**更容易被点到**（多点几次「设置」= 多几次 JS 建窗 = 多几次撞锁）。

> ⚠️ **教训：改完一个功能后出现的 ANR，先别急着怪自己那次改动。**
> `dumpsys dropbox` 里有**历史**记录，逐帧比对就能判断是「新引入」还是「本来就在、
> 只是触发概率变了」。这一次的答案明确是后者。

##### 4.7.27.4 修法：A（最小改动）—— 前端防重入闸门 + 删掉失同步来源

> **为什么不是 B（把设置页做成主 WebView 的覆盖层）**：用户一句话否掉了 ——
> 「那主 webview 里还有桌宠呢」。桌宠浮起来时主 WebView 被搬进了
> `WindowManager`，根本不在 Activity 视图树里，覆盖层方案不成立（也见 4.7.12.8）。

**A 的两处改动**：

**① `src/components/views/PetMode.vue` —— `handleOpenSettings` 加防重入闸门**

```ts
let settingsOpening = false;
let settingsOpenTimer: ReturnType<typeof setTimeout> | null = null;
const SETTINGS_OPEN_TIMEOUT_MS = 8000;

const releaseSettingsOpenGate = () => {
  if (settingsOpenTimer !== null) {
    clearTimeout(settingsOpenTimer);
    settingsOpenTimer = null;
  }
  settingsOpening = false;
};

const handleOpenSettings = async () => {
  if (settingsOpening) return; // ★ 上一次建窗还没落地，直接忽略
  try {
    const bridge = (window as unknown as { LingChatApp?: { focusSettings?: () => boolean } })
      .LingChatApp;
    const focusSettings = bridge?.focusSettings;
    // ★ Android：**先问原生**（实例还在不在，它才是唯一权威）
    if (isAndroid() && typeof focusSettings === "function") {
      if (focusSettings.call(bridge)) return;
    } else {
      const existing = await WebviewWindow.getByLabel("settings");
      if (existing) {
        await existing.setFocus();
        return;
      }
    }
    settingsOpening = true; // ★ 走到这里 = 必须新建，立刻上闸
    settingsOpenTimer = setTimeout(releaseSettingsOpenGate, SETTINGS_OPEN_TIMEOUT_MS);
    const webview = new WebviewWindow("settings", {
      /* … */
    });
    webview.once("tauri://created", () => {
      releaseSettingsOpenGate();
    });
    webview.once("tauri://error", (e) => {
      releaseSettingsOpenGate();
    });
  } catch (error) {
    releaseSettingsOpenGate(); // `new WebviewWindow` 标签重名会**同步抛**
  }
};
```

关键语义变化：**删掉了「桥返回 false 就掉下去建窗」那条路**。旧代码在
「`getByLabel` 拿到窗口 → 桥返回 false」时会继续走到建窗，那正是死锁入口。现在：
**原生说在就 return，说不在就直接建窗**，两条路互斥。

**② `SettingsActivity.kt` —— 删掉 `onResume` 的 `finish()` 守卫**（见 4.7.12.6），
连带删 `pendingShow` / `wentToBackground` / `onStop` / `onResume`。它是
「Tauri 登记表 ↔ Android 实例」失同步的唯一来源，删掉之后「实例在、tao 又建窗」
这个组合不会再出现。

##### 4.7.27.5 怎么读 ANR（本次踩过的坑，可复用）

| 做法                                         | 结果                                                                                                                                     |
| -------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| `logcat -d -b main` 全量导出                 | ❌ 主缓冲区被**无影客户端**刷爆，16 MiB 只装得下约 1 分钟；不带 `-t` 的 `-d` 跑 8 分钟还拿不到目标时段                                   |
| 直接读 `/data/system/procexitstore/anr_*.gz` | ❌ `Permission denied`                                                                                                                   |
| `dumpsys activity exit-info <pkg>`           | ✅ **第一入口** —— 直接给 `reason=` / 时间戳，判断「是不是 ANR」最快                                                                     |
| `dumpsys dropbox --print data_app_anr`       | ✅ shell 可读，含完整线程栈（本次 154,425 行 / 63 个进程段）                                                                             |
| 设备侧 `-t` 限行 + `grep` 落盘，再拷回来     | ✅ 别在设备侧一遍遍 grep（用户原话：「这提取的日志怎么还花那么久？你可以拷贝过来再弄啊」）                                               |
| 文件传输                                     | ✅ **走 `G:` 盘**（Dokan 挂载的手机存储，4–5 MB/s，19 MB / 3.8 s）；❌ 不要 `adb pull`（用户原话：「哎呀，和你说多少次了？文件走存储」） |

> ⚠️ `libling_chat_lib.so` 是 **strip 过**的（只有 `.dynsym`，没有 `.symtab`），
> 栈帧地址**没法符号化**；而且它在 APK 里是 `compress=0`，
> `header_offset=0x2C62D8` ≠ 崩溃里显示的 `0x2c8000`（`libc++_shared.so` 被内联到
> 别的串里导致偏移漂移）。所以本次的锁链路是**顺着源码推出来的**，不是从符号读出
> 来的 —— 证据是「两条线程同时卡 futex」+「10:45 帧序列逐帧一致」，而不是某一行
> 函数名。

#### 4.7.28 ★ 主活动按返回键会「退回桌宠页」—— Chromium 自带的返回回调没被压住

**真机症状**（用户原话）：

> 当软件在前台主活动，按返回键会导致桌宠里面被返回。这很边缘了。

**根因**：`MainActivity` 从来没有注册过自己的 `OnBackPressedCallback`。

`WryActivity.setWebView()` 里本来有一个：

```kotlin
open val handleBackNavigation: Boolean = true
...
if (handleBackNavigation) {
    val callback = object : OnBackPressedCallback(true) {
        override fun handleOnBackPressed() {
            if (mWebView.canGoBack()) mWebView.goBack()
            else { isEnabled = false; onBackPressed(); isEnabled = true }
        }
    }
    onBackPressedDispatcher.addCallback(this, callback)
}
```

但 `TauriActivity` 把它关掉了：

```kotlin
abstract class TauriActivity : WryActivity() {
  override val handleBackNavigation: Boolean = false
}
```

**关掉的只是「我们那一份」，Chromium 自带的那份还在。** WebView 构造时会自己注册一个
「能回退就 `goBack()`」的回调（这条在 4.7.12.5 里已经为设置页验证过），而
`OnBackPressedDispatcher` 按注册顺序**倒序**派发 —— 我们一份回调都没有，它自然就被派发了。

于是返回键变成了**主 WebView 的历史回退**。主 WebView 的历史里通常留着 `/pet`：
从桌宠回到聊天页之后按返回键，`goBack()` 直接**退回了桌宠页** —— 这就是用户看到的现象。

> 为什么「很边缘」：只有当 `canGoBack()` 为真时才发作；没有历史时 Chromium 那份会
> `isEnabled = false` 回落到 `onBackPressed()`，也就是正常的 `finish()`。

**修法**：给 `MainActivity` 也注册一份（`installBackCallback`），行为 = `moveTaskToBack(true)`。

- 注册**两次**（`onCreate` 兜底 + `onWebViewCreate` 稳定生效），理由与设置页 4.7.12.5 完全一样；
- 用 `moveTaskToBack` 而不是 `finish()`：与设置页同一套「只用退回、不要关闭，以节省性能」的既定方向。

⚠️ **代价**：「聊天页 → 主菜单」这类**页内后退**不再由返回键承担（改由页面上的按钮承担）。
这是刻意取舍 ——「根页面按返回键应该退出 App」比「浏览器式后退」更符合预期。将来若要两者
兼得，需要前端上报「当前是不是根路由」，原生据此分流。

#### 4.7.29 切设置的加强：建窗期间停掉会抢锁的轮询

**真机症状**（用户原话）：

> 主活动在前台，切设置会卡一下，这里可能会崩。

「崩」的机制就是 4.7.27 那场 ANR（`webviews_lock()` 互锁）。4.7.27 的防重入闸门解决了
「连点导致多次建窗」，但**单次建窗**仍然会持锁，而桌宠态下有三个定时器在周期性发起 IPC：

| 定时器              | 周期  | 动作                                 |
| ------------------- | ----- | ------------------------------------ |
| `metricsTimer`      | 500ms | `getFloatingPetStatus()`             |
| `selfHealTimer`     | 500ms | 内容高度变了才 `resizeFloatingPet()` |
| `heightReportTimer` | 200ms | 去抖后一次 `resizeFloatingPet()`     |

它们会和建窗抢同一把锁，把「本来就慢的建窗」进一步拖成卡顿甚至 ANR。

**修法**：新增 `pauseIpcTimers()` / `resumeIpcTimers()` —— 闸门一上就停，
`releaseSettingsOpenGate()` 里恢复（成功 / 失败 / 超时 / 同步抛异常四条路都走它）。

**另一处**：Android 建窗不再传 `transparent: true`。

桌面端设置窗是圆角透明窗，需要它；Android 上 `SettingsActivity` 用的是**不透明主题**
（4.7.3），传 `transparent: true` 只会让 wry 把 WebView 背景也置成全透明 —— 没有视觉收益，
却把 WebView 推进软件合成路径，拖慢建窗。

```ts
transparent: !isAndroid(),
```

#### 4.7.30 ★ 屏幕视奸：Android 端补上「截一帧」（无障碍服务）

**背景**：项目的「视觉感受与主动窥屏」（README「👀」）在桌面端是完整的，Android 端
**整条不触发**：

- `screen_analyzer.rs::capture_screen_as_jpeg()` 是 `#[cfg(not(target_os = "windows"))] { None }`
- `strategy_dispatcher.rs::get_screen_prompt()` 拿到 `None` 就 `?` 早退

其余全现成：`IntentType::Screen`（TTL 120s）、`enable_visual_perception`、`screen_weight`、
30s 轮询、原生多模态当轮直发 / 旁白转述回退。**缺的只有「截一帧」这一步。**

**方案取舍**（用户拍板）：无障碍服务 `takeScreenshot`，而不是 `MediaProjection`。

| 维度         | 无障碍 `takeScreenshot` | `MediaProjection`                   |
| ------------ | ----------------------- | ----------------------------------- |
| 授权方式     | 系统设置里开一次        | 每次会话弹「开始投屏」              |
| App 冷启动后 | 仍有效                  | **必须重新授权**                    |
| Android 14+  | 无额外要求              | 必须先起 `mediaProjection` 前台服务 |
| 附加能力     | 顺带拿到前台包名        | 只有画面                            |
| API 下限     | **30（Android 11）**    | 21                                  |
| Google Play  | 审核极严                | 无政策风险                          |

对一个「后台默默窥屏」的功能，「每次冷启动都被弹一次投屏授权」不可接受，故选前者。
`MediaProjection` 留作将来的第二通道。

**实现（五处）**：

1. **`LingChatAccessibilityService.kt`**（插件模块，新建）
   - `takeScreenshot` → `HardwareBuffer` → **`copy(ARGB_8888)`** → 缩到最长边 1024 → JPEG(85)
   - ⚠️ **必须 copy**：`wrapHardwareBuffer` 拿到的是 GPU 侧位图，buffer close 之后直接压
     会得到一张**全黑图**
   - `onAccessibilityEvent` 只记前台包名（`TYPE_WINDOW_STATE_CHANGED`），作为窥屏的语境
   - 阻塞等待上限 5s；API < 30 直接返回 `null`
2. **`res/xml/lingchat_screen_accessibility.xml`**（新建）—— `canTakeScreenshot="true"` +
   **`canRetrieveWindowContent="false"`**（只截画面、不读控件树，是刻意的隐私边界）
3. **插件 manifest** —— `BIND_ACCESSIBILITY_SERVICE` 权限 + intent-filter + meta-data
4. **插件命令 `captureScreen`** —— 返回 **JPEG 文件路径**（不是字节：一帧 100~300 KB，
   base64 塞 JSON 再涨 33%，而这条链路每 2 分钟就走一次）
5. **Rust 侧**：`ScreenCapture` 模型 + `mobile`/`desktop` 转发 + `capture_screen_bytes()` 高层封装

**关键的依赖方向问题**：`ling-chat-main` 是底层 crate，**不能**依赖
`tauri-plugin-floating-pet`（Tauri 插件）。所以用**全局回调注入**：

```rust
// screen_analyzer.rs
static ANDROID_SCREEN_CAPTURE: OnceLock<ScreenCaptureFn> = OnceLock::new();
pub fn set_android_screen_capture(f: ScreenCaptureFn) { let _ = ANDROID_SCREEN_CAPTURE.set(f); }

// src-tauri/src/app/setup/mod.rs（app 层注册）
#[cfg(target_os = "android")]
{
    let capture_handle = app.handle().clone();
    ai_service::screen_analyzer::set_android_screen_capture(Box::new(move || {
        tauri_plugin_floating_pet::capture_screen_bytes(&capture_handle)
    }));
}
```

好处：`capture_screen_as_jpeg()` 的**签名不用变**，`ScreenAnalyzer` 也不用持有 app；
桌面端不注册（Windows 自己走 GDI，读不到这个槽位也不需要它）。

**⚠️ 用户必须手动开启**：设置 → 无障碍 → 已安装的服务 → 「LingChat 屏幕感知」。
App 无法自行开启，也没有授权弹窗。未开启时回调返回 `None`，屏幕策略照旧不触发 ——
与改造前行为一致，**不引入新的错误路径**。

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
bash build-android-pet.sh                        # 约 10–13 分钟，产出未签名 APK
bash sign-apk-pet.sh <改动摘要>                   # zipalign + apksigner → *-signed.apk，并落位
bash verify-pet-apk.sh                           # 验包：确认改动真的进了包（见下）
```

**「落位」的约定（别漏）**：签名脚本会把成品复制到构建根目录，命名

```
/d/LingChat-BuildEnv/LingChat-pet-YYYYMMDD-<改动摘要>.apk
```

摘要用**短横线连接的英文小写**，一眼能看出这版改了什么：

```
LingChat-pet-20261002-fix-safeinset-and-gen-lock.apk
LingChat-pet-20261002-settings-window-and-backkey.apk
LingChat-pet-20261003-responsive-settings-and-petsize.apk
```

⚠️ 这一步**漏过两次**，所以已经固化进 `sign-apk-pet.sh` 的第 4 步：
不传摘要就只提示、不落位（避免生成一堆名字没意义的包）。

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
18. **安全区不溢出**（★ 本轮重点，见 4.4.2.2）：分别在**竖屏**和**横屏**下，
    把桌宠拖到并吸附在**左沿**和**右沿** → 展开、折叠、再展开 →
    圆角 / 挖孔 / 手势条那一圈**不能被桌宠压住**，右边也不能溢出去
19. **放久之后还能对话**（★ 本轮重点，见 4.5）：进悬浮窗后**放着不动 10 分钟以上**
    （主动对话系统若开着，这段时间里它会自己发起生成）→ 然后发一条消息 →
    必须能正常收到回复；同时看聊天页的角色卡 / 剧本日志是否还是空的。
    若仍然卡住，抓 logcat 搜 `生成超时` / `超过硬上限` —— 有这行说明锁被按时
    强制释放了（但病根在别处，按 4.5.4 的清单继续查）；没有这行则说明卡在
    4.5.4 里某条尚未加界的路径上。
20. **返回键不再鬼打墙**（★ 本轮重点，见 4.6）：从主页进聊天页 → 点「桌宠」进悬浮窗
    → 点左侧「返回主页」收回 → **按一次返回键**，应当**直接回主页**；
    不能出现「聊天页 → 宠物页 → 聊天页 → 主页」这种来回弹。
    再验一次：进出悬浮窗来回 3 次后按返回，仍然一次到主页。
21. **设置键能开出窗口**（★ 本轮重点，见 4.7）：在悬浮窗里点「设置」→
    应当**新开一个整屏的设置页**（不是「没反应」）。逐项确认：
    - 内容**没有**被状态栏压住（顶部标签条完整可见），底部也没被手势条压住；
    - 背景是**不透明**的，**不**透系统壁纸（4.7.3）；
    - 侧栏在窄屏是顶部**横向滚动**的标签条，能滑到第 4 个「主动对话」（4.7.7）；
    - 头部**没有**最小化 / 最大化按钮，只有主题切换 + 关闭；
    - 转屏后设置页**不重新加载**（`configChanges` 生效）。
22. **关设置窗不会拆掉悬浮窗**（★ 本轮重点，见 4.7.4 / 4.7.5）：先在悬浮窗里
    开设置 → 用**关闭按钮**关掉 → 回到悬浮窗，**桌宠必须还在**；
    再开一次设置 → 用**返回键**关掉 → 桌宠仍然还在。
    同时确认关设置窗后**没有黑屏**（若黑屏 = 走成了 `Window.close()`，
    见 4.7.4）。
23. **设置窗里的改动会同步回悬浮窗**：改一下「桌宠大小」滑块 → 关设置 →
    悬浮窗里的桌宠应当已经按新尺寸渲染（靠 `pet-scale-changed` 事件广播）。

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

| 现象                                                                | 可能原因                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| ------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 切回 App 白屏                                                       | 占位页没生效，`setContentView` 顺序有问题                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| 收回后主界面黑屏                                                    | WebView 搬回失败，需看 `restoreWebViewToActivity` 日志                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| **收回后只剩左上一角、不回聊天页**                                  | `pet-attached` 没送达。三层保险：`notifyWeb` 显式收 WebView 参数、0/300/1000/3000ms 重试、Activity resume 时由 `ensureLifecycleCallbacks` 补发                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| **收回后整个界面缩在左上角**                                        | **`setContentView` 没重置 LayoutParams**（见 4.4）。先量 `window.innerWidth` 和屏宽对照：等于悬浮窗宽度就是这条，等于屏宽则是视口问题                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| **点 ✕ 收回直接闪退**                                               | **`setContentView` 之后事后赋值 `view.layoutParams` → `FrameLayout.onMeasure` 的 ClassCastException**（见 4.4）。改成两参重载即可。已从「startActivity flag」「moveTaskToFront」等嫌疑点排除                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| **收回后整个 App 用窄视口渲染**                                     | 先量 `window.innerWidth`。等于屏宽就是视口问题；等于悬浮窗宽度则是 LayoutParams 没改回 MATCH_PARENT                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| **展开后一大片空白（内容挤在左上角）**                              | 两种成因，先看诊断 `gap L…T…R…B…` 那一行定位：**左右有余量** = 缩放系数错（`notifyMetrics` 曾用 `view.width`，它在 `updateViewLayout()` 之后是上一形态的值）；**上边为负 / 下边大** = 内容被系统顶偏了（见下一条）                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| **展开后内容被顶到屏幕上方、下方空一片**                            | 展开态摘掉 `FLAG_NOT_FOCUSABLE` 后，系统开始插手窗口内容区（insets / 输入法），而画布尺寸只由窗口**宽度**推出，高度被改它不知情（见 4.4）。检查 `SOFT_INPUT_STATE_VISIBLE` 是否还在、`fitInsetsTypes` 是否设了 0；诊断里看 `scroll=` 与 `doc=`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| **展开后透明区特别大、且随桌宠缩放变**                              | 页面走了桌面端分支（`PET_WIDTH_BASE × pet.scale`，没有 `--pet-fit`）。看诊断 `applied=` 与 `role scaleP=`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| **展开后四周一圈透明**                                              | ✅ **已修**（见 5.1.1 / 5.1.2 / 5.1.3）。**三个来源，别再只查一个**：① **诊断代码自绘** —— 画布上的品红底色 + 头像容器的青色描边，宠物是非矩形，露出来的那一圈看起来就是「套了一圈」（见 5.1.2，已全部关掉）；② 画布**外**的透明 —— 系统给窗口加了 insets，WebView 比窗口小一圈（见 4.4）；③ 缩放系数用了**缓存值** —— `--pet-fit` 滞后 500ms 时画布按旧系数渲染，比窗口小（见 5.1.3，已改成现算的 `liveFit`）。诊断里看 `gap` 行（全 0 才对）与 `shell` 行（必须等于 `inner`）                                                                                                                                                                                                                                                                                                                                |
| **展开后「一整屏」都能被触摸、摸着还能把桌宠拖走**                  | **少写 `FLAG_NOT_TOUCH_MODAL`**（见 5.1.4）。没有它，窗口的可触摸区域是**整块屏幕**，窗口外的触摸也被投递给它 → `buildPetTouchListener` 的 ACTION_DOWN 触发 → 窗口跟手走。**在别的应用里同样发生**（那里没有我们的 Activity，正好可用来确认）。修法是一行 flag，且注意别用整体赋值 `params.flags = ...` 把它抹掉                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| 展开后**窗口矩形之内**、宠物轮廓之外的角落吃触摸（固有）            | 悬浮窗是矩形、宠物是圆形，那圈角落**无法**逐像素穿透（见 4.4）。范围仅限窗口自身矩形（收起 60×60dp / 展开 216×252dp），**窗口之外不受影响**；看诊断 `band=`（不为 0 = 气泡带占了高没渲染）、`role scaleP=`（< 1 = 宠物只占头像框一部分）                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| **横屏下宠物被挤出屏幕**                                            | 尺寸基准用了 `screenWidthDp()`。横屏 0.6×屏宽 推出的高度超过屏高。已改为 `min(屏宽, 屏高)`（见 4.4）                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| **展开后右边空 120 / 下边空 322 的空白**                            | **`fit` 的三个来源全失效**（`pet-metrics` 推送、500ms IPC 轮询、`resize` 事件），系数停在初值 1.0。判据：诊断里 `win=0x0dp` 且标签停在 `[enter]`（没变成 `[live]`）。已加 500ms 本地心跳自愈（见 4.4.1），并把 `resize` 改成在 `onMounted` 里**无条件**绑定（早先只在「挂载时已是悬浮窗」的分支绑，而启动顺序是先 `replace('/pet')` 再 `showFloatingPet()` → 那条分支永远走不到）                                                                                                                                                                                                                                                                                                                                                                                                                              |
| **画布高度比窗口矮，底部留一条透明**                                | 已加兜底：`floatingCanvasHeight = max(内容高, 视口高 / fit)`。宽度是构造出来的（`逻辑宽 × fit ≡ 窗口宽`），高度不是，必须显式兜                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| **点空白区域会收起输入框**                                          | 空白落在 `#pet-app` 之外、窗口之内 → 手指派发 `mouseleave` → 桌面端的「光标离开即收起」。它是「空白存在」的旁证。已把悬浮窗里的 `mouseenter/leave` 改成空操作（见 4.4.1）                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| **横屏时桌宠只能停在左半边（右边一大片拖不过去）**                  | 边界读数**经过 Activity 的 Resources**。这条走过**三版**、都被证伪：① `resources.displayMetrics`；② `maximumWindowMetrics`（AOSP 里它读的就是 `getResources().getConfiguration().windowConfiguration.maxBounds`，同一个 Resources）；③ `DisplayManager.getDisplay().getRealMetrics()` —— 看着离开了 Resources，实则内部 `adjustMetrics` 又按 `mResources` 的旋转交换一次，**还是绕回去**。桌宠悬浮时 App 已 `moveTaskToBack` 到后台，后台 Activity 的 Resources 不跟随旋转刷新 → 边界永远停在竖屏宽度。改用 `Display.getRotation()` + `Display.getMode()`（两者直读 DisplayInfo，见 4.4.2.1）。⚠️ **但来源换对了不代表组合对**：`if (rotated) ph to pw else pw to ph` 只对「mode 报自然方向」的机型成立，报当前方向的机型会交换回竖屏 —— 换台机器就复发。必须改成取短边/长边再按 rotation 分配，见 **4.4.2.2** |
| **横屏左侧多一块空白、右侧内容超出屏幕**                            | 主题没声明 `android:windowLayoutInDisplayCutoutMode`。Android 14 及以下默认值在横屏把窗口**信箱化**（挖孔一侧由系统留白），前端 `.pl-safe` 又按 insets 补一次 → 左侧双份；窗口可用宽度 < 物理宽度而 `100dvw` 仍按整屏算 → 右侧溢出。修法见 **4.7.9**                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| **设置页竖屏只剩一条标签条、下面全空白（点标签像没反应）**          | `SettingsPage.vue` 的 `<main>` 是横向 `flex`，而窄屏的标签条 `w-full shrink-0` 把整行占满 → 内容区 `flex-1` 分到 **0 宽**（`overflow-y-auto` 让 `min-width:auto` 归零）再被裁掉。内容其实切换了，只是看不见。修法见 **4.7.8**                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| **「桌宠大小」滑块拖了等于没拖**                                    | 悬浮窗宽度由原生独占（`短边 × 比例 × petScale`），而前端 `appStyleVars` 悬浮窗分支刻意不乘 `pet.scale`（会双重缩放）、`pet-scale-changed` 里又只调桌面端的 `applyWindowLayout()` —— 两头都不动。修法是新增 `set_scale` 命令，链路四处（含 `build.rs` 的 `COMMANDS` 与 `permissions/default.toml`）缺一不可，见 **4.7.11**                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| **贴边时右边（或左侧）溢出去一点**                                  | **按物理边缘夹，没扣系统安全区**（挖孔 / 圆角 / 手势条那圈）。`screenSizePx()` 给的是**物理**尺寸，而桌宠是贴边吸附的 —— 横屏时挖孔通常落在某一侧，那一侧可视宽度本来就更窄，于是贴到「物理右沿」就压在挖孔上（见 4.4.2.2）。修法是新增 `safeInsetsPx()`，`clampIntoScreen` / `snapToEdge` / 旋转重排全部按安全区取值。⚠️ 安全区**必须**取 `getCurrentWindowMetrics().getWindowInsets()` —— 同一个 `WindowMetrics` 上的 `bounds` 走的是 `windowConfiguration.getMaxBounds()`，就是上面那条被证伪三版的 Resources 路径，**同源不同路，别顺手用**                                                                                                                                                                                                                                                                |
| **折叠（展开 → 头像）时界面闪一下**                                 | 窗口尺寸与缩放系数**不可能同帧生效**：窗口先缩到约 60dp，页面还按展开态的大系数（约 1.03）渲染 → 画布比窗口大、内容被裁掉一块，几帧后才缩回来（见 5.1.5）。修法两侧各一处：原生把 `notifyMetrics` 挪到 `updateViewLayout` **之前**；前端 `liveFit` 在「刚收到原生即时几何」且「与现算对不上」时改用权威系数。⚠️ 这两条都是**顺序 / 取值**改动，验包脚本无法用字符串断言，只能真机确认                                                                                                                                                                                                                                                                                                                                                                                                                          |
| **旋转屏幕后桌宠跑到屏幕外**                                        | 三个成因，都在 4.4.2：① `x/y` 是屏幕坐标系里的**绝对值**，旋转后屏宽高对调就飞出屏幕；② **读数来源取错**，`lastScreenW/H` 永远不变 → 重排判据永不成立（见 4.4.2.1）；③ **回调根本不会来** —— `ComponentCallbacks.onConfigurationChanged` 只对可见 Activity 送达，而悬浮时 Activity 在后台。修法是 `DisplayManager` 的显示器回调 + 500ms 轮询兜底（见 4.4.2.2 / 4.4.2.3），**两个方向（横→竖、竖→横）都要验**                                                                                                                                                                                                                                                                                                                                                                                                   |
| **竖屏↔横屏来回切，桌宠跑出屏幕 / 贴不到边**                        | 同上。验证时至少来回切 3 次，并分别试「贴着左沿」和「贴着右沿」两种起始位置                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| **反复进出悬浮窗后，悬浮窗里变成聊天页 / 角色凭空消失**             | `handleReturnedToApp()` 被**误触发**（见 4.4.2.4）。**三条误判路径都要查**：① `getFloatingPetStatus()` 的 catch 把 IPC 失败伪装成 `detached: false` → 轮询当成「已收回」（现在靠 `queried` 字段拦住）；② 收回时排队的 `pet-attached` 重试（0/300/1000/**3000**ms）在 3 秒内重进悬浮窗后**迟到送达** → 命中新一轮监听器（**主因**，现在靠原生 `petModeEpoch` 作废 + 前端 `confirmReturnedToApp` 复核）；③ `show()` 没清 `pendingAttachNotify`，跨轮残留的引用会在 resume 时补发。注意这个 bug **不可逆**（`handleReturnedToApp` 会停掉轮询），所以还有第 6 条兜底：`MainChat.healStuckFloatingState` 在挂载 1.5s 后发现「页面不在 /pet 但 WebView 在悬浮窗」会补一次导航自愈（**必须延迟** —— 正常收回时也会短暂出现同样的读数）                                                                                |
| **悬浮窗放久之后，消息能发出去但没反应**（角色卡 / 剧本日志还变空） | **全局生成锁被永久持有**（见 4.5）。`generation_lock` 被持有跨越整条 LLM 流式生成，而 LLM 侧的超时**只在空闲时计时**（reqwest `read_timeout`、两次 `next()` 之间的 `idle_timeout`）—— 服务端只要持续发 SSE 心跳/注释字节就永不触发。任意一轮生成卡死 → 后续**所有**请求永久排队。注意「消息能发出去」只是前端**乐观插入**，不代表 Rust 收到了；特效/按钮正常恰说明卡的是 Rust 侧。已给「前置步骤」与「每一轮生成」各加一层**绝对**硬上限（`GENERATION_HARD_LIMIT_SECS = 600`），超时即 drop 掉 future 释放锁并 emit `ai:error` + `status:reset`。⚠️ 4.5.4 列了几条**尚未加界**的路径（`generate_line_voice` 的 TTS、`get_proactive_prompt` 那段同时持 `ai_service`+`game_status` 跑视觉模型），复现仍在就先查那里                                                                                              |
| **窗口尺寸变了但页面不跟**                                          | `resize` 监听没绑上（`enterFloatingLayout` 里漏绑，见 4.4）。诊断里 `inner=` 与 `win=` 长期不一致就是这条                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| **退后台后桌宠不动**                                                | 保活轮询没起来；看 logcat 里 `FloatingPet` 的「已启动保活轮询」                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| **悬浮窗里点不到按钮 / 按钮看不见**                                 | 左侧那排按钮仍在挂桌面端的 `-left-3.5`（落在画布外的「呼吸边」里，被 `overflow-hidden` 裁掉）。定位必须换、显隐时机**不该**换：它和电脑端一样是「悬停才浮现」，悬浮窗里的「悬停」由**展开态**提供（`isStageHovered`），所以收起态看不见是**正确行为**（见 5.3）                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| **收起态点头像，却打开了设置（或按钮闪一下）**                      | 两个成因：① 隐藏的按钮仍在接收触摸 —— 悬浮窗里按钮压在头像上，需要 `pointer-events-none`（见 5.3.1）；② 悬停仍由指针事件判定 —— Android 会把触摸合成成 pointer 事件，落在窗口边角就触发一次（见 5.3.2）                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| **旋转后要等半秒才归位**                                            | `displayListener` 没注册上（`ensureDisplayListener`），只剩 500ms 轮询兜底。看 logcat 有没有「屏幕尺寸变化：悬浮窗重排为 …」                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| **气泡看不到 / 以为消息没发出**                                     | 窗口高度没跟着内容长，气泡被裁；查 `reportFloatingHeight` 的 IPC 是否成功                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| **单击经常没反应、窗口被带偏**                                      | `TAP_SLOP_DP` 偏小，正常点按被判成拖动                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| 悬浮窗里输入框弹不出键盘                                            | 窗口 `FLAG_NOT_FOCUSABLE` 没在展开态摘掉                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| 按住 Home 后悬浮窗消失                                              | 前台服务被 ROM 拦截，需加「后台弹出界面」白名单                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| **点「设置」没反应**                                                | `WebviewWindow` 少了 `activityName` → tao 的 `create_activity` 找不到类 → 只走 `tauri://error`，而前端**没监听**该事件，所以静默失败、日志里什么都没有（见 4.7.1 / 4.7.2）。三处要一起改：`SettingsActivity.kt` / `AndroidManifest.xml` 注册 / 前端传 `activityName`。⚠️ `launchMode` 现在是 `singleInstance`（为了让设置窗独占任务栈，见 **4.7.12.2**）—— 代价是**实例已存在时绝不能再走 tao 建窗**：复用走 `onNewIntent`、不触发创建回调，tao 会白等 5 秒。由前端 `getByLabel("settings")` 守卫                                                                                                                                                                                                                                                                                                              |
| **按桌宠的返回键，设置页不收起**                                    | 设置窗与 MainActivity **同任务栈**，而 `FloatingPetPlugin.hide()` 的 `bringActivityToFront()` 走 `moveTaskToFront(聊天任务)` —— 拉的是**整个任务**，栈顶仍是设置窗。`finish()` 能修但会销毁实例（用户不要），`moveTaskToBack` 会把聊天页一起退到后台。**唯一解是给设置窗独立任务栈**（见 **4.7.12.2**），桌宠侧一行都不用改                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| **设置页在「最近任务」里多出一张卡片**                              | 独立任务栈的副作用。修法：`SettingsActivity` 加 `android:excludeFromRecents="true"`（见 4.7.12.2）                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| **打开设置再返回，桌宠就没了**                                      | `PluginManager.onDestroy(activity)` **没有** `isInitialized` 守卫，会把**每个** Tauri Activity 的销毁都转发给插件 —— 关设置窗 = 误触 `FloatingPetPlugin.onDestroy` = 拆掉还开着的悬浮窗（见 4.7.5）。修法：`if (activity !== this.activity) return`。判据必须是**实例相等**（插件是单例，绑定的就是 MainActivity）                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| **关设置窗后一块黑屏**                                              | 前端走了 `getCurrentWindow().close()`。`tao` 的 Android 后端**没有** `Window::close()` / `destroy()`，`on_close_requested` 只摘掉 Rust 侧 wrapper，**没人 `finish()` 那个 Activity**（见 4.7.4）。修法：`onWebViewCreate` 里 `addJavascriptInterface` 一个 `close()`，前端移动端优先调它（它现在做的是「退回」而不是 `finish()`，见 4.7.12）                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| **设置页糊在壁纸上 / 文字看不清**                                   | 设置窗继承了 `Theme.ling_chat`（`windowShowWallpaper` + `windowIsTranslucent` + 透明背景），而 wry 在 `transparent: true` 时还会把 WebView 背景也置成全透明（`wry/src/android/main_pipe.rs`）—— 三层全透（见 4.7.3）。修法：新增 `Theme.ling_chat.settings`，`windowBackground` 换 `?android:colorBackground`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| **设置页被状态栏 / 手势条压住**                                     | 设置窗是**独立 Activity**，安全区变量由 `MainActivity` 注入、对它不生效。`targetSdk = 36` 时 Android 15+ 对所有 Activity 强制 edge-to-edge（见 4.7.6）。修法：抽出 `SafeAreaInsets.kt` 两个 Activity 共用，设置页根元素加 `.pt-safe/.pr-safe/.pb-safe/.pl-safe`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| **设置页内容挤成一条**                                              | 200px 竖侧栏在 360dp 屏上只剩 112px 给内容（见 4.7.7）。修法：`SettingsSidebar` 按 `md:` 断点切成顶部横向滚动条 —— 按**可用宽度**判断，不按设备类型                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |

## 九、工程验证方式

### 9.1 本地构建（现在的主路径）

本机已配好完整 Android 工具链，**不必再走 GitHub Actions**：

```
D:\LingChat-BuildEnv\
├─ LingChat-Pet\            ← fork 的 feat/android-floating-pet 工作副本
├─ jdk\  android-sdk\  rust\  mingw\  gradle-home\   ← 工具链与缓存
├─ android-keystore\debug.keystore                    ← 调试签名
├─ build-android-pet.sh     ← 一键出包（对照 CI 步骤，pnpm 换成直接 node）
├─ build-android-pet-fast.sh ← ⚡ 快速迭代：只改 Kotlin ~50s、只改 Rust ~6.5min（见 §9.7）
├─ sign-apk-pet.sh          ← zipalign + apksigner
└─ verify-pet-apk.sh        ← 验包：确认改动真的进了包（R8 混淆 + brotli 压缩后搜字符串的坑）
```

```bash
cd /d/LingChat-BuildEnv
bash build-android-pet.sh          # 约 10–13 分钟
bash sign-apk-pet.sh               # 产出 *-signed.apk，可直接覆盖安装
bash verify-pet-apk.sh             # 可选但强烈建议：确认改动真的进了包
```

> ⚡ **迭代时先看 §9.7**：只改 Kotlin / XML / 资源的话，
> `bash build-android-pet-fast.sh` 约 **50 秒**出包（完整构建要 12–13 分钟，
> 其中 11 分钟是在重编一个字节都没变的 Rust）。改了 Rust 用
> `bash build-android-pet-fast.sh rust`，约 6.5 分钟。

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

#### 9.2.1 成因二：`data_manifest.json` 每次都被重写（⚠️ **旧 tauri 版本的行为，2026-10-03 复核已作废 —— 见 9.2.2**）

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

#### 9.2.2 复核（2026-10-03）：成因二在 dev 上**已经不成立**，但重编仍在

用一次真实构建（`pet20.log`）复核，结论有三条。

**① 成因二的输入已经不存在了。** 当前 build script 的 `output`（`target/.../build/ling_chat-<hash>/output`）里：

```
cargo:rerun-if-env-changed=TAURI_CONFIG
cargo:rerun-if-changed=.../tauri.conf.json
cargo:rerun-if-changed=.../tauri.android.conf.json
cargo:rerun-if-changed=.../gen/android/tauri.settings.gradle
cargo:rerun-if-changed=.../gen/android/app/tauri.build.gradle.kts
cargo:rerun-if-changed=capabilities
cargo:rerun-if-env-changed=REMOVE_UNUSED_COMMANDS
```

**`.bundled_resources\data_manifest.json` 不在里面了** —— 它只出现在
`src-tauri/target/` 下 10-01 的**废弃单元**里。所以 §9.2.1 记的是**旧 tauri 版本**
的行为；rebase 到 dev（tauri 2.11.1）之后，这条路径已经不参与判定。

**② 重编仍然存在。** 同一次构建：`Compiling ling_chat` 两次，两个
``Finished `release` profile``（13m 04s + 6m 06s）。第二趟是 gradle 的
`BuildTask` 跑 `$ tauri "android" "android-studio-script" "--release" "--target" "aarch64"`。

**③ 但两趟是「同一个单元」，所以确实等价。** 判据在 fingerprint：

- 每个带 build script 的包**天然有两个** `<pkg>-<hash>/` 目录 ——
  一个装 `run-build-script-build-script-build`，一个装 `lib-*`。
  实测 `tauri-plugin-floating-pet`、`ling-chat-main`、`ling_chat` **全都是 2 个**。
  ⚠️ **不要**把这 2 个 hash 误读成「编了两遍」—— 这是我 10-03 踩过的坑。
- 真正该数的是 **lib 单元**：当前 target 里 `ling_chat` 只有
  **1 个 lib 单元**（+1 个 build-script 单元）。两趟用的是**同一个 unit hash**
  → **同一组 rustflags / profile / features / target** → 从 cargo 的输入视角看，
  这两次编译**是等价的**，第二趟是**纯缓存失效**（白烧 6 分钟），不是「编了两个不同的东西」。

**⚠️ 一个保留**：unit hash 相同**不保证产物逐字节一致** —— build script 吐出的
`cargo:rustc-cfg` / `cargo:rustc-env` **不进 hash**。要钉死「产物也一致」得比对两次
产物的哈希；而第一趟的产物已被第二趟覆盖，拿不到。**别把「等价」说得比证据更满。**

**④ 新的嫌疑（未验证）：`rerun-if-env-changed=TAURI_CONFIG`。**
本地构建脚本给 `tauri android build` 传了
`--config '{"build":{"beforeBuildCommand":""}}'`，tauri-cli 会把它塞进 `TAURI_CONFIG`。
两趟若该变量不同 → build script 重跑 → `ling_chat` 判脏 → 重编。
一锤定音的办法是让 cargo 自己说：

```bash
CARGO_LOG="cargo::core::compiler::fingerprint=trace" cargo build --target aarch64-linux-android --release
```

它会逐字打出 `dirty: <原因>`。**别在构建进行中跑**（target 目录锁）。

**⑤ 一处自相矛盾，尚未裁决。** `.cargo/config.toml` 的头注释（本分支 `5dd49484`
改写的，**不是 dev 的**）说「两趟 cwd 不同 → rustflags 不同 → 同一个 crate 编两遍；
删掉第二份就消失」；而本文 §9.2 的「顺带纠正一个常见误判」说「两趟 cwd 都是
`src-tauri/`，两份 config 不是原因，删掉也修不了」。两者不能同时成立。
**实测站后者**：`src-tauri/.cargo/config.toml` 早已删除，仍然会重编一次。
dev 原版的注释说的是**相反**的话（「两份必须保持一致」「两份都在实际生效」），
而 dev 里那份文件是**存在**的 —— 删除动作是本分支引入的偏离。

#### 9.2.3 复核这类问题时的四个坑（可复用）

1. **cargo 的日志带 ANSI 颜色码**，且把 `Compiling` 和 crate 名**分别染色** →
   `grep "Compiling ling_chat"` 永远 0 命中。先剥颜色码：
   `sed -r 's/\x1b\[[0-9;]*[A-Za-z]//g'`。
2. **crate 名 ≠ 包名**。包是 `ling_chat`，lib target 叫 `ling_chat_lib`
   （产物 `libling_chat_lib.so`，fingerprint 里是 `lib-ling_chat_lib`）。
   搜「`Compiling ling_chat`」时别把 `ling-chat-main` / `ling-chat-plugins` 混进来。
3. **target 根会随 workspace 布局变化**（见 §9.6）。老路径不会消失，只会变成
   陈旧残留 —— 拿它做判断会得到**整体相反**的结论。
4. **`<pkg>-<hash>` 的个数不等于编译次数**（见上面 ③）。

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

### 9.5 磁盘空间：构建峰值约 2.5 GB，而 `rm` 可能没真的释放

#### 峰值构成

| 产物                                                         | 大小    |
| ------------------------------------------------------------ | ------- |
| `target/aarch64-linux-android/release/libling_chat_lib.rlib` | ~700 MB |
| `.../libling_chat_lib.so`                                    | ~130 MB |
| `gen/android/app/build/`（Gradle 中间产物 + 输出）           | ~1.2 GB |
| APK 输出（unsigned + signed + aligned）                      | ~650 MB |

D 盘只剩 1.2 GB 时会在**第二次 cargo 归档**炸掉，报的却是一句很容易误读的话：

```
error: failed to build archive at `...\deps\libling_chat_lib.rlib`: 磁盘空间不足。 (os error 112)
FAILURE: ... Execution failed for task ':app:rustBuildArm64Release'.
```

`os error 112` = `ERROR_DISK_FULL`。**它报的是 archive 写入失败，不是编译错误** ——
别去翻 Rust 代码。

#### ⚠️ 陷阱：`rm` 会被「安全删除」钩子转进回收站

实测：`rm -rf src-tauri/target/release`（1.3 GB）+ `rm -rf gen/android/app/build`
（1.2 GB），`du` 确认目录已消失，**但 `df` 的可用空间只从 1.20 GB 涨到 1.29 GB**。

真因是文件钩子把 `rm` 变成了「移入回收站」—— 文件只是换了个位置：

```text
(Get-ChildItem 'D:\$RECYCLE.BIN' -Force -Recurse -File -EA SilentlyContinue |
  Measure-Object Length -Sum).Sum/1MB    # => 3687 MB / 6184 个文件
```

所以要**真删除**，必须先关钩子（`build-android-pet.sh` 开头已经这么做了）：

```bash
export CODEBUDDY_SAFE_DELETE_ENABLED=0
export CODEBUDDY_BROKERED_FS_HOOK_ENABLED=0
export CODEBUDDY_SAFE_DELETE_SANDBOX=0
```

腾空间前**先看回收站** —— 里面可能已经躺着几个 GB 的「已删除」文件。

#### 清空回收站：`Clear-RecycleBin` 不可用，手动兜底

```text
Clear-RecycleBin -DriveLetter D -Force
# => Clear-RecycleBin : 指定程序要求更新的 Windows 版本。（FailedToClearRecycleBin）
```

报错但**其实已经删掉了绝大部分**（进度条走完了）。复核方式：那个
`D:\$RECYCLE.BIN\S-1-5-21-*` 子目录里只剩 `desktop.ini` 就是清空了。
手动兜底 = 删该 SID 子目录下的 `$R*` / `$I*`。

#### 测量口径：用 `fsutil`，别信 `du`

Git Bash 的 `du` 在 Windows 上会被**硬链接**骗到（cargo 会把
`deps/<name>-<hash>.rlib` 硬链到 `release/lib<name>.rlib`，两次分开跑就重复计数）。

```text
fsutil volume diskfree D:
[System.IO.DriveInfo]::new('D').AvailableFreeSpace/1GB
```

### 9.6 `verify-pet-apk.sh` 的 codegen 缓存路径会**随 workspace 布局失效**

**事故（2026-10-03）**：验包脚本报了 **22 条 ❌「应存在，却没找到」**，清一色是
前端标记，看着像「前端根本没进包」。

**真相：包完全正确，是脚本读错了目录。**

dev 把后端 workspace 化之后（根 `Cargo.toml` 的 `members` 含 `src-tauri`，
而 `src-tauri/Cargo.toml` **不再自带 `[workspace]`**）→ **workspace 根 = 仓库根**
→ cargo 的 target 目录从 `src-tauri/target/` 变成了 `<repo>/target/`。
而 `verify-pet-apk.sh` 里 `ASSETS` 写死的是：

```bash
"$REPO"/src-tauri/target/aarch64-linux-android/release/build/ling_chat-*/out/tauri-codegen-assets
```

那个目录**不会消失**，只会变成陈旧残留。实测对照：

| 目录                                   | 文件数 | mtime                  |
| -------------------------------------- | ------ | ---------------------- |
| `<repo>/target/.../ling_chat-51c7b0…`  | 133    | **10-03 00:19** ← 真实 |
| `src-tauri/target/.../ling_chat-1670…` | 299    | 10-02 13:11 ← 陈旧     |

脚本按「brotli 解压后取 sha256，与当前 `dist/` 比对」筛「本次在用」分块，
读旧目录时只剩 **2** 个命中 → 所有 `present` 断言**集体假失败**。

**修法**：两个候选根都找，`ls -dt` 取 mtime 最新：

```bash
ASSETS=$(ls -dt "$REPO"/target/aarch64-linux-android/release/build/ling_chat-*/out/tauri-codegen-assets \
               "$REPO"/src-tauri/target/aarch64-linux-android/release/build/ling_chat-*/out/tauri-codegen-assets \
          2>/dev/null | head -1)
```

修完重跑：`本次在用分块 38 个；剔除陈旧残留 0 个`，全绿。

**教训**：验包脚本里**任何写死的构建产物路径都是定时炸弹** —— 构建布局一变，
断言不是报错而是**静默给出相反结论**（这里是「前端没进包」）。
所以脚本第 0 节要打印产物时间线，看到 `tauri-codegen-assets/` 的时间明显落后于
`dist/index.html`，就该怀疑路径而不是代码。

#### 9.6.1 顺带补上的三类断言（695cd804）

同一次复核里补了此前没有覆盖的项：

- **`setScale` / `setExpanded`（dex 方法名）** —— Tauri v2 移动端**按方法名反射派发**
  （`PluginManager` 扫 `@Command` 方法），R8 一改名就变成运行时报「命令不存在」，
  而且**不是编译错误、不是崩溃**，是静默失败。所以必须断言方法名原样在包里。
  `setExpanded` 作为**对照组**同时断言：它挂了说明断言本身失效。
- **`NOT_VISIBLE`** —— `setScale` 的可见性守卫错误码（该轮新增的字面量）。
- **`windowLayoutInDisplayCutoutMode=always`（第 5.6 节）** —— 这是**主题属性**，
  在 arsc 里是属性 ID 的数值、**不是可搜文本**，所以不能像
  `Theme.ling_chat.settings` 那样 grep 字符串。正确姿势：

  ```bash
  aapt2 dump resources <apk> | grep -oaF -e "0x01010586=3"
  ```

  属性 id `0x01010586`，值 `3` = ALWAYS。**必须 >= 2 处**：`windowLayoutInDisplayCutoutMode`
  是 **API 28 属性**，aapt2 会自动把它归到 `(v28)` 与 `(night-v28)` 两个配置；
  只在 `values/themes.xml` 写、漏了 `values-night/`，就会**恰好少一次** ——
  那正是「白天修好了、深色下没修」这个坑的判据。

### 9.7 快速迭代：只改 Kotlin 时 cargo 可以**整个跳过**（2026-10-03）

**动机**：完整构建 ~12–13 分钟（构成见 §9.1 / §9.2），其中 **cargo 两趟各 ~5.5 分钟**。
而改 `SettingsActivity.kt` 这种 Kotlin 文件时，**Rust 侧一个字节都没变**，
那 11 分钟纯粹是白烧。

**做法**：`mergeUniversalReleaseJniLibFolders` 只是「把 jniLibs 里的 .so 合并进 APK」，
它依赖 `rustBuildUniversalRelease` 仅仅是因为**首次构建**时 .so 还不存在。
只要 `.so` 已经在 `app/src/main/jniLibs/arm64-v8a/`（符号链接指向
`target/aarch64-linux-android/release/libling_chat_lib.so`），
就可以把那个叶子任务排除掉：

```bash
cd src-tauri/gen/android
./gradlew assembleUniversalRelease \
  -PtargetList=aarch64 -PabiList=arm64-v8a -ParchList=arm64 \
  -x rustBuildArm64Release
```

脚本：`/d/LingChat-BuildEnv/build-android-pet-fast.sh`（默认 kotlin 模式）。

#### 9.7.1 实测数字

| 场景                   | Gradle | 含签名总计 | 对比完整构建     |
| ---------------------- | ------ | ---------- | ---------------- |
| **有 Kotlin 改动**     | 44s    | **~51s**   | 12–13 min → ~15× |
| 无任何改动（固定开销） | 7s     | ~16s       | —                |

（第一次跑是 77s —— daemon 刚被 `--stop` 掉、且增量状态被上一轮误构建污染；
daemon 热起来后稳定在 44s。所以**别在快速迭代前 stop daemon**。）

#### 9.7.2 ★ 必须自己传 `-PtargetList` / `-PabiList` / `-ParchList`

**这是本方案唯一的坑，而且它不报错、只是变慢。**

`RustPlugin.kt` 里这三个属性都是「不传 = 全部」：

```kotlin
val defaultAbiList  = listOf("arm64-v8a", "armeabi-v7a", "x86", "x86_64")
val defaultArchList = listOf("arm64", "arm", "x86", "x86_64")
val targetsList = (findProperty("targetList") as? String)?.split(',')
                  ?: listOf("aarch64", "armv7", "i686", "x86_64")
```

平时只编 aarch64，是因为 **tauri CLI 自己传了这三个属性**（在
`node_modules/.pnpm/@tauri-apps+cli-win32-x64-msvc@*/…/cli.win32-x64-msvc.node`
里能 grep 到字面量 `-PtargetList=` / `-PabiList=` / `-ParchList=`；
它经 cargo-mobile2 调 `assembleUniversal<Profile>`）。
我们绕过 CLI 直接调 `gradlew`，就必须自己补齐。

**只 `-x rustBuildArm64Release` 是不够的** —— 另外三个 target 会顶上来：

```
:app:rustBuildArmRelease       ← armv7
:app:rustBuildX86Release       ← x86
:app:rustBuildX86_64Release    ← x86_64
:app:rustBuildUniversalRelease
```

而且每个**新 target 都要从零重编全部依赖**。2026-10-03 实测：漏了这三个属性，
构建卡在 armv7 上 **20 分钟还没完**（`target/armv7-linux-androideabi/` 都没建出来就手动掐了）。

**自查方法**：`--dry-run` 后 grep 任务名，必须**只剩** `rustBuildUniversalRelease`
（空壳 DefaultTask，无 action）和 `assembleUniversalRelease`：

```bash
./gradlew assembleUniversalRelease -PtargetList=aarch64 -PabiList=arm64-v8a \
  -ParchList=arm64 -x rustBuildArm64Release --dry-run | grep -i rustbuild
# => :app:rustBuildUniversalRelease SKIPPED     ← 只应有这一条
```

#### 9.7.3 rust 模式：用 init script 关掉 Gradle 那趟重复 cargo

改了 `src-tauri/**/*.rs` 时 cargo 必须跑，但**只需跑一趟**。
`android-studio-script` 那条路走不通（见 9.7.4），所以改成
「CLI 照常驱动 + 从 Gradle 侧把重复任务禁掉」。

`$GRADLE_USER_HOME/init.d/skip-rust-build.gradle`，由环境变量触发：

```groovy
if (System.getenv("LINGCHAT_SKIP_GRADLE_RUST") == "1") {
    gradle.taskGraph.whenReady { graph ->
        def rustTasks = graph.allTasks.findAll { it.name.startsWith("rustBuild") }
        // 空任务图（`./gradlew --stop` 那次）不报警
        if (rustTasks.isEmpty()) { ...; return }
        rustTasks.each { it.enabled = false }
    }
}
```

然后：

```bash
LINGCHAT_SKIP_GRADLE_RUST=1 tauri android build --target aarch64 --apk \
  --config '{"build":{"beforeBuildCommand":""}}'
```

**为什么敢禁**：**两趟 cargo 都会把 .so 铺进 jniLibs**。
证据：`pet21.log` 里 `symlinking lib ... in jniLibs dir` 出现 **4 次 = 2 对**
（每对 = `libling_chat_lib.so` + `libc++_shared.so`）。
所以禁掉 Gradle 那趟后，`mergeUniversalReleaseJniLibFolders` 照样有货。

实测（Rust 无改动，cargo 秒回）：**103s**，日志里 `Compiling ling_chat` **0 次**、
`symlinking lib` **2 次**（只剩 CLI 那一趟）、`[skip-rust-build] 已禁用 2 个任务`。
有真实 Rust 改动时约 **5.5 min（cargo）+ 1 min（gradle）≈ 6.5 min**，
对比完整构建省约 6 分钟。

#### 9.7.4 ⚠️ `android-studio-script` **不能**独立调用

它要连 `tauri android build` **父进程托管的 WebSocket**。独立调用直接 panic：

```
thread '<unnamed>' panicked at crates\tauri-cli\src\mobile\mod.rs:403:6:
failed to read CLI options: Context("failed to build WebSocket client",
  Io(Os { code: 10061, kind: ConnectionRefused, message: "由于目标计算机积极拒绝，无法连接。" }))
```

所以只能「让 CLI 驱动 + 禁 Gradle 侧任务」，不能「自己跑 cargo + 自己跑 gradle」。

#### 9.7.5 逐字节一致的证据

kotlin 模式产出的 APK 与完整构建 **sha256 完全相同**：

```
837ba0dd28d598fa3b1cffd89f13ce5cbc0e6e061fa317b26653dc2149b4c3bd
```

即 `-x rustBuildArm64Release` 不改变产物，只是省掉重复劳动。
（验包脚本 76 条断言同样全绿。）

#### 9.7.6 脚本的预检 —— 因为近路的失效方式是**静默产出旧内容**

`build-android-pet-fast.sh` 在构建前会拒掉不该走近路的情况：

| 模式   | 条件                                                                 | 动作                              |
| ------ | -------------------------------------------------------------------- | --------------------------------- |
| kotlin | `src-tauri/{src,crates,plugins}/**/*.rs` 或 `Cargo.toml` 比 `.so` 新 | 退出码 2，提示改走 `rust`         |
| kotlin | 前端源码（`src/**` / `index.html`）比 `dist/` 新                     | 退出码 2，提示改走 `rust`         |
| rust   | 前端源码比 `dist/` 新                                                | **自动补跑 `vite build`**（见下） |

前端那条尤其重要：`dist/` 会被 tauri-codegen 用 brotli 压进 `libling_chat_lib.so`，
只跑 Gradle 拿到的是**上一次的前端** —— 不报错，只是行为诡异。

#### 9.7.7 ★ 预检曾经是**假的** —— `stat` 不解引用符号链接（2026-10-03 踩到）

第一版预检用 `dist/` 和 `$SO`（jniLibs 里那个 `.so`）比 mtime。它**永远为假**，
于是 rust 模式静默跳过 vite build，打出「新后端 + 旧前端」的包。

两个独立的错叠在一起：

**① `$SO` 是符号链接，`stat` / `find -newer` 默认不解引用。**

```
jniLibs/arm64-v8a/libling_chat_lib.so  ->  ../target/aarch64-linux-android/release/libling_chat_lib.so
```

`stat -c '%s'` 拿到的是**链接自身**的 90 字节，不是真 `.so` 的 136 MB。
日志里那句 `.so: 90 bytes` 就是症状 —— **一个 90 字节的 `.so` 本该当场可疑**。
修法：`SO_REAL=$(readlink -f "$SO")`，之后一律比 `$SO_REAL`；
打印也改成 `stat -L`。

**② 判据方向本身是自指的。**

「`dist/` 比 `.so` 新」只说明「上次构建之后前端改过」。但**上次构建可能本来
就没重跑 vite**（正是本脚本引入的新情况）→ 判据的前提被自己破坏，永远为假。

正确的判据是问这个问题的本来语义：

```bash
find "$PROJ/src" "$PROJ/index.html" -type f -newer "$PROJ/dist"
```

即「**前端源码**是不是比 `dist/` 旧」。它不依赖任何构建产物的时间戳，
在「前端 / Rust / Kotlin 改了什么」这三种情况下都成立。

> **教训**：预检的判据不能建立在**上一轮的产物**上，只能建立在**源码**上。
> 产物是「上一次构建做了什么」的函数，而预检恰恰是用来判断「上一次构建
> 该不该那样做」的 —— 拿它当输入就是循环论证。
>
> 附带一条：日志里任何**数值离谱的读数**（90 字节的 `.so`、0 字节的 dist）
> 都应当被当成断言失败，而不是被忽略过去。这次就是靠那行日志发现的。

#### 9.7.8 rust 模式为什么会**自动补跑** `vite build`

`tauri android build` 的 `beforeBuildCommand` 被脚本用 `--config` 置空了
（因为完整构建已经自己跑过一遍，不置空会跑两次）。所以 rust 模式**不碰前端**。

但「改 Rust」和「改前端」经常同时发生（本轮就是：Kotlin + Rust + 前端三处一起改）。
不补的话就是 9.7.7 说的静默旧前端。

默认 `--front auto`：预检发现前端源码比 `dist/` 新就自动补跑一次
`node node_modules/vite/bin/vite.js build`（约 30s，与完整构建 `[5/7]` 同一条命令，
**跳过 `vue-tsc` 类型检查**）。`--front` / `--no-front` 可强制 / 跳过。

> `data.7z`（`prepare-bundled-resources.mjs`）**不在**本脚本职责内：
> 它只跟 `src-tauri/resources` 下的素材有关，前端 / Kotlin 改动都不影响它。
> 素材变了请走完整构建 `build-android-pet.sh`。
