# 场景光影（2.5D 多光源）数据结构

本文说明 `LightingParams` 新增的五组字段、它们与现有 CSS 滤镜的关系，以及渲染接入计划。

## 现状：两条渲染路径并存

dev 上的光影仍是 CSS 滤镜加径向渐变叠加层（`GameBackground.vue` / `GameRoleAvatar.vue` 里的 `filter` 与 `radial-gradient`）。这条路能整体压亮度，但认不出角色轮廓——拉暗就是整图变灰，补亮就像手电筒照脸。

新路线是原生 WebGL 逐像素着色。两套并存的原因是旧场景存档里只有 CSS 字段，新场景才会带上 WebGL 参数。`LightingParams` 用可选字段区分：缺省即走旧路径，有值才走新管线。

## 新增字段

五组字段全部可选，`None` 时不参与序列化，旧场景回写后 JSON 形状不变。

### lights：多光源数组

上限 8 盏。四种灯型共用一套扁平字段，不适用的字段由前端填默认值，避免给 serde 上标签枚举。

| 字段                    | 说明                                                                            |
| ----------------------- | ------------------------------------------------------------------------------- |
| `id` / `name`           | 编辑器内的标识与显示名，前端生成                                                |
| `is_key`                | 主光标记，纯 UI 语义，不参与光照计算                                            |
| `type`                  | `point`（点光）/ `directional`（平行光）/ `spot`（锥光）/ `ambient`（环境底光） |
| `enabled`               | 单灯开关，缺省 true                                                             |
| `color` / `intensity`   | 颜色与强度，全部灯型通用                                                        |
| `x` / `y` / `z`         | 舞台归一化坐标与高度，点光与锥光使用                                            |
| `radius` / `softness`   | 照射半径与边缘软硬，点光与锥光使用                                              |
| `azimuth` / `elevation` | 方位角与仰角（度），平行光与锥光使用                                            |
| `cone` / `penumbra`     | 锥角与锥边半影，锥光使用                                                        |

### global：整体强度与调色

`master` / `ambient_blend` / `albedo_protect` / `soft_knee` / `lift` / `mono`。

对应着色器里的总强度、环境混合比例、反照率保护、软膝色调映射、暗部抬升与去色。

调色组吸收旧 CSS filter 的能力，让 2.5D 路径不再丢功能：

| 字段                            | 范围            | 作用                                                                                           |
| ------------------------------- | --------------- | ---------------------------------------------------------------------------------------------- |
| `saturation`                    | 0–2，默认 1     | 饱和度，0 = 灰度，>1 增强                                                                      |
| `contrast`                      | 0–2，默认 1     | 对比度                                                                                         |
| `tint_hue` / `tint_amount`      | −180–180° / 0–1 | 整体色调偏移，替代旧界面的「暖色调 / sepia」                                                   |
| `char_exposure` / `bg_exposure` | 0–2，默认 1     | 立绘与背景各自的曝光增益，与 `master` 相乘。旧界面能分开调角色和背景亮度，这两个字段是它的替代 |

### compositing：合成层

`light_wrap`（光绕过轮廓包裹暗侧）、`rim_width`（边缘光宽度）、`bloom_int` / `bloom_thresh`（泛光强度与阈值）、`glow_color`（泛光染色，默认 `#ffffff` 即不染色）。

旧的 overlay 径向渐变异色层不走这里，仍由 `overlay_*` 与 `light_x` / `light_y` / `blend_mode` 驱动 —— 那组字段本来就存在，只是 2.5D 路径还没消费它们。

### material：材质

`use_normal` / `use_depth` 是总开关，关掉后整条伪法线 / 伪深度管线跳过，回落到只有多光源 RGB 增益的表现。

其余为 `normal_strength`（法线强度）、`cel_thresh` / `cel_softness`（卡通着色分界与过渡）、`spec_int` / `spec_gloss`（高光强度与锐度）、`self_shadow_int`（自阴影）。

### camera：景别

`shot` 取 `fs` / `ws` / `ms` / `cu` / `ecu` 五档，配合 `zoom`、`focus_y`（垂直构图焦点）、`dof`（背景景深）。

## 兼容性

`LightingParams` 是手写 `Deserialize`，同时处理两种历史格式：滤镜字段在顶层的旧格式，和带 `character` / `background` 嵌套的新格式。五组新字段在两个分支里都读取，任一格式缺失都落到 `None`。

`global` 与 `compositing` 的新增字段都靠容器级 `#[serde(default)]` 取值，旧存档读出来自然落到默认值（饱和度 / 对比度 / 曝光 = 1，色调偏移 = 0，`glow_color` = 白），行为与引入前一致。

**注意两处同名不同层**：

- `character.saturation` / `background.saturation` 是旧 CSS filter 的单图层参数，只在旧路径生效；
- `global.saturation` 作用于 WebGL 合成后的整幅画面，只在 2.5D 路径生效。

`character.glow_color` 与 `compositing.glow_color` 同理：前者是旧 CSS 的 drop-shadow 颜色，后者是泛光染色。两条路径互斥，不会叠加。

四个子结构用 `#[serde(default)]` 配手写 `Default`，因此只给部分字段时缺失项取合理默认值，而不是 `0` / `false`。例如 `material` 只写 `normal_strength` 时，`use_normal` 仍是 `true`。

## 渲染接入计划

本提交只落数据结构，不接渲染。后续按能力分层推进，每一层单独立 PR：

| 阶段       | 内容                                        |
| ---------- | ------------------------------------------- |
| 渲染内核   | WebGL 着色器、8 灯循环、伪法线 / 伪深度材质 |
| 合成与指标 | 泛光、景深、量化指标计算                    |
| 编辑器面板 | 三栏工作台、参数控件、灯心拖拽              |
| 挂载与预设 | 接入主舞台、内置预设参数                    |

每层再按 CONTRIBUTING 二·2 的 500 行建议细分，保证每个 PR 能独立回滚。具体拆几个视实现体量决定，这里不预先写死。
