# 标准视图 · Live2D 组件层级与层叠问题分析

> 关联图：[stage-components.html](diagrams/stage-components.html)（组件调用链 / z-index 层叠 / 数据通道 / 问题热区）
> 适用范围：标准聊天视图（`MainChat → GameRolesStage → Live2DStage` 链）；桌宠与设置预览仅作附注。
> 行号以 2026-08 分支 `pr-665` 为准，改动代码前请重新核对。

## 一、组件层级概述

### 1. 挂载链

```
App.vue → <router-view> → CompanionMode.vue → MainChat.vue
```

标准视图唯一入口是 `/chat`。主菜单（MainMenu）也内嵌 `MainChat`，但点"自由对话"会 `router.push('/chat')`，真实挂载仍走上述链。**没有 KeepAlive** 包裹游戏舞台。

### 2. MainChat 子级渲染顺序与 z-index

`.main-box` 为 `position:absolute; width/height:100%; overflow:hidden`，**无 z-index → 不建层叠上下文**，因此所有子级 z-index 直接解析到页面根层叠上下文。

| 渲染顺序 | 组件 | 定位 | z-index |
| --- | --- | --- | --- |
| 1 | FreeModeTools | — | auto |
| 2 | GameBackground | absolute | 背景图 `-2`，粒子 `0`（isolate 内 `114514`） |
| 3 | **GameRolesStage** | absolute | 内部见下 |
| 4 | GameDialog | relative | `2`（隐藏态 `-1!`） |
| 5 | #menu-panel | fixed | `1000` |
| 6 | GameExtraUI | fixed | `999` |
| 7 | ImageSourcePicker / LoadingTransition | — | 全局覆盖 |

### 3. GameRolesStage

根 div `absolute w/h-full overflow-hidden`，**z:auto → 不建层叠上下文**。子级：

- `<Live2DStage class="z-2" ...>`，slot 内 `RoleAvatar v-for`
- 光照叠加层 `absolute inset-0 z-10`（`mix-blend-mode` 径向渐变，整屏）
- `<audio ref="mainAudio">`

### 4. Live2DStage：fragment 双根

`defineOptions({ inheritAttrs: false })`，模板是两个根节点：

1. `<div v-bind="$attrs" ref="host" class="absolute inset-0 ...">` —— `class="z-2"` 经 `$attrs` 转发**只落到 host div**（`z-2` 建层叠上下文），Pixi canvas 以 `absolute inset-0` 追加进 host。
2. `<slot>` —— RoleAvatar 是 host 的**兄弟节点**，在页面根上下文，**不在 z-2 内**。

`provide(live2dStageContextKey)` 提供 `{ readyRoleIds, unavailableRoleIds }`（`ReadonlySet<number>`）。

### 5. GameRoleAvatar：fragment 三兄弟

根节点是三个兄弟（无包裹容器）：

1. `Live2DRolePresentation v-if="role.live2d"` / `StaticRolePresentation v-else` —— 静态图 `z:1`
2. `TouchAreas v-if="command==='touch'"` —— `fixed z:2`
3. effects div（含 `.bubble`）—— `z:2`，`.bubble` 自身 `z-index:2`

`roleLayerStyle`（left / top / `translateX(-50%) scale()` / opacity / transition / filter）被复制进 `staticLayerStyle`（z:1）与 `effectsLayerStyle`（z:2）两份。

### 6. 层叠上下文小结

| 元素 | 是否建上下文 | 说明 |
| --- | --- | --- |
| `.main-box`（MainChat） | 否 | 无 z-index / transform / filter |
| GameRolesStage 根 | 否 | absolute + z:auto |
| Live2DStage host div | **是** | `z-index:2`，canvas 被困在 z:2 |
| GameBackground 粒子包装 | **是** | `isolation:isolate`，内层 z:114514 不逸出 |
| GameBackground 带 filter 的包装 div | **是** | 见"附注 ②" |

## 二、问题清单

### P1. 混合 Live2D + 静态角色深度错乱，且无逐角色深度控制

**现象**：同一场景所有 Live2D 立绘**恒在所有静态立绘之上**；想表达"某个 Live2D 角色在某个静态角色后面"的场景纵深做不到；Live2D 模型之间只有 `presentRolesList` 数组顺序可排，无法单独调整前后。

**根因**：全部 Live2D 画在同一个共享 Pixi canvas，canvas 所在 host div `z-index:2`（一个层叠上下文）；静态立绘 `staticLayerStyle.zIndex:'1'` 恒在 z:2 之下。z 值按**技术来源**（Live2D vs 静态）划分，而不是按**场景深度**划分。模型间的先后只能靠 Pixi `addChild` 顺序（即数组序），不能与静态角色统一排序。

**修复建议**：为角色引入场景深度字段 `sceneZ`，让 host div 的 z-index 与静态层的 z-index 由**同一把刻度**计算（如 `10 + sceneZ`），Live2D 与静态共用。**不要把静态角色迁入 Pixi**（`development.md` 明示反对），只统一 z 刻度即可。

### P2. 气泡盖在模型上方靠 DOM 顺序巧合

**现象**：`.bubble z-index:2` 与 host div `z-index:2` 同为 2，气泡能显示在模型上方**仅仅因为** DOM 中 host 在前、气泡在后。一旦调整 DOM 顺序（或引入新的层叠上下文），模型就会盖住气泡。

**根因**：同一层叠上下文内同 z-index，绘制退化为 DOM 顺序，没有显式约定"气泡必须在模型之上"。

**修复建议**：effects 层显式 `z-index:3`（高于 host 的 2），并加注释写明意图；或在 `GameRolesStage` 给 `Live2DStage` 外包一层 `isolation:isolate`，把整个 canvas 上下文沉到 0 级，与角色层彻底解耦。

### P3. `class="z-2"` 只转发到 host div，而非舞台根

**现象**：`<Live2DStage class="z-2">` 的 z-2 因 `inheritAttrs:false` + `v-bind="$attrs"` 落到**内部 host div**，而组件是 fragment、无根元素可承接。z-2 只包住 canvas；slot 里的立绘在页面根上下文。目前"碰巧可用"，是因为 MainChat `.main-box` 与 GameRolesStage 根都不建层叠上下文——一旦这些祖先出现 `filter / transform / isolation / z-index`，canvas 与立绘会分层崩坏。

**根因**：fragment 无根元素 + `$attrs` 转发，class 布局语义落在错误的元素上。

**修复建议**：Live2DStage 显式声明 `props.zIndex`（或调用方用 `:style` 传），host 用 `:style="{ zIndex }"`；在组件内注释"本组件不承接 class 布局，z-index 必须显式传入"。

### P4. GameRoleAvatar 三碎片根 + 样式重复两份

**现象**：`GameRoleAvatar` 是 fragment 三兄弟（presentation / TouchAreas / effects+bubble）。`roleLayerStyle` 被复制成 `staticLayerStyle`(z:1) 与 `effectsLayerStyle`(z:2) 两份，任何布局改动必须同步两处，存在漂移风险；气泡定位依赖与图片**同一套** left/top/transform；`character-fade` 过渡只包住图片，气泡/特效层不同步（表情切换时图片淡入但气泡突兀出现）。

**根因**：为了省一个包裹 div 用 fragment，把同一角色的定位/缩放/过渡拆到三个孤立节点。

**修复建议**：加一个**不设 transform** 的内部容器 div（`absolute inset-0 pointer-events-none`）承载 presentation 与 bubble，`roleLayerStyle` 只算一份传给容器；TouchAreas 保持 fixed 兄弟；过渡（character-fade）作用于容器，使图片与气泡同步。

### P5. 父子 / 归属语义倒置

**现象**：Live2DStage 通过 slot 在视觉上"包含"角色 DOM，但模型归 stage、DOM 归 role，两者归属方向相反；就绪态经 `provide/inject` 向下传，又经 `activeChange` / `failedChange` 向上 emit，**同一信息两个方向**；`Live2DRolePresentation` 是"纯 StaticRolePresentation 包装器 + `visible` 开关"，把"渲染静态"与"是否显示"混在一个组件里。此外 `GameRoleAvatar` 里的 `presentationRef` 是 `Live2DRolePresentation | StaticRolePresentation` 联合类型，靠 `v-if/v-else` 二选一后调用 `waitForLoad()`，是类型层面的妥协。

**根因**：职责归属没有单一方向，契约定义过薄（`development.md` 只禁止 `GameRolesStage` 复制模型结果，未明确定义"stage 拥有模型、role 拥有 DOM"）。

**修复建议**：在代码注释与文档中固化契约："**stage 拥有模型、role 拥有 DOM**"；把 `readyRoleIds` / `unavailableRoleIds` 改名为 `modelReadyRoleIds` / `modelFailedRoleIds` 强调是模型状态；`visible` 推导留在 `StaticRolePresentation` 内，`Live2DRolePresentation` 只负责"ready → 隐藏静态、unavailable → 兜底文案"。

### P6. 光照叠加层 z-10 盖住对话框

**现象**：`overlay_target` 为 `character` / `both` 时，`GameRolesStage` 内 `absolute inset-0 z-10` 整屏径向渐变**盖住 GameDialog(z:2)**，对话框文字也被染色。`character` 目标的灯光本意只照角色，却无法限定。

**根因**：单一整屏 overlay 一层 + z-10 高于对话框，只能作用于屏幕矩形。

**修复建议**：把字符光照并入每角色 `layerStyle.filter`（复用现有 `lightingFilter`，它已支持 brightness/contrast/saturate/glow/sepia）；或 `overlay_target === 'character'` 时把 overlay 的 z 降到 `1`（静态之上、气泡/对话框之下）并裁剪到角色包围盒。

### P7. 隐藏对话框 `z-[-1]!` 仍在背景之上

**现象**：GameDialog 隐藏态用 `z-index:-1 !important`，仍高于背景图（`z:-2`）。

**根因**：`-1 > -2`。

**修复建议**：隐藏态改为 `z-[-3]` 或直接用 `v-show=false` 卸载。低优先。

### P8.（可选）桌宠 / 标准组件重复

**现象**：`src/components/pet/GameRolesStage.vue` 与 `pet/GameRoleAvatar.vue` 是标准版（`game/standard/`）的独立副本，气泡/音频/表情解析逻辑重复；且桌宠 Live2D 就绪态用 props（`live2d-active` / `live2d-failed`）而非 `provide/inject`，与标准版两套机制。

**根因**：桌宠独立窗口与布局，早期复制未抽象。

**修复建议**：抽取公共 `useRoleVisual(role)` composable 供标准/pet 复用；Live2D 就绪态统一走 `provide/inject`。成本较高，属长期重构。

## 三、修复优先级汇总

| 编号 | 标题 | 影响面 | 成本 | 优先级 |
| --- | --- | --- | --- | --- |
| P1 | 混合 Live2D + 静态深度错乱 | 视觉正确性 | 中 | 高 |
| P2 | 气泡盖模型靠 DOM 顺序巧合 | 稳定性 | 低 | 高 |
| P3 | `z-2` 转发陷阱（fragment） | 隐性崩塌 | 低 | 高 |
| P6 | 光照 overlay 盖住对话框 | 视觉正确性 | 低 | 中 |
| P4 | 三碎片根 + 样式重复两份 | 可维护性 / 动画一致 | 中 | 中 |
| P5 | 归属语义倒置（provide/inject 反向） | 可维护性 | 中 | 中 |
| P7 | 隐藏对话框 `z-1` 高于背景 | 轻微 | 低 | 低 |
| P8 | 桌宠 / 标准组件重复 | 可维护性 | 高 | 低 |

**一句话修复**：P1/P2/P3 本质是"层叠刻度缺失 + 依赖 DOM 顺序 + fragment 落点错误"三件事，建议一次统一：给 Live2D 与静态共用一个 z 刻度（`10 + sceneZ`）、气泡层显式 +1、Live2DStage 用 `props.zIndex`。

## 附注

1. **`role-container-transition` 未定义**：`GameRoleAvatar.vue:25` 与 `StaticRolePresentation.vue:4` 都引用了 `role-container-transition` 类，但整个 `src/` 下没有任何地方定义它（`grep role-container-transition src/` 仅两处引用）。疑似遗留的 Tailwind/旧类名，当前不产生实际效果。
2. **GameBackground 的 filter 包装 div 额外建上下文**：背景包装 div 带 `filter` 时会独立建层叠上下文，把 `.game-background z:-2` 关进 0 级上下文内（与粒子 `isolation:isolate` 同理）。属于潜在陷阱：未来若给背景加更多 filter，-2 的相对基准会变化。
3. **`staticLayerStyle` / `effectsLayerStyle` 重复**：见 P4。改布局时若只改一处，气泡会与立绘错位。
