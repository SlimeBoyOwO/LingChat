# 工具权限管理（permission）

> 权限模型图见 [diagrams/permission.html](diagrams/permission.html)。

## 1. 权限模型：二维矩阵

权限不是「单个工具开关」，而是 **「场景组 × 角色组」二维矩阵**，由三张表组合而成（`ToolPermissionConfig`，见 [diagrams/permission.html](diagrams/permission.html) 的类图）：

```
ToolPermissionConfig
├── scene_mapping: HashMap<GeneratorSourceKey, String>   // ① 调用来源 → 场景组名
├── scene_groups:  HashMap<String, ToolPermission>       // ② 场景组权限
└── role_groups:   HashMap<String, GroupPermission>      // ③ 角色组权限（角色必须归属某个组）
```

- **场景维度（谁来调）**：调用来源（`GeneratorSourceKey`，共 5 种）经 `scene_mapping` 映射到某个场景组；场景组是一份 `ToolPermission`（`enabled` / `tools` 集合 / `all_tools` 开关）；
- **角色维度（谁在调）**：角色名归属某个角色组（`GroupPermission`，比场景组多一个 `roles` 集合，记录组里有哪些角色）；角色**必须归属某个组**才可能拿到权限；
- **最终结果 = 场景组 ∩ 角色组**：角色实际可用工具集 = 场景组允许的工具 ∩ 角色组允许的工具（另有 `all_tools` 短路逻辑，见 §5）。

`GeneratorSourceKey` 与生成管线的 `GeneratorSource` 一一对应：

```rust
UserChat · Proactive · ScriptAiDialogue · ScriptFreeDialogue · EntryGreeting
```

## 2. 配置文件：`tool_permissions.toml`

- 位置：`<data_dir>/tool_permissions.toml`（`data_dir()`，与数据库同目录）；
- 首次启动由 `load_or_create(data_dir, 全部工具名)` 自动生成，之后每次启动加载；
- 写入是**原子写**（临时文件 `.tmp` → `rename`），与剧本编辑器同一套防坏盘策略；
- **当前没有 Tauri 命令暴露给前端**：权限配置在「Rust 侧 API + 手改 TOML」两个层面管理（本 PR 是 Part 0，前端管理界面属后续工作）。

生成的默认配置形如：

```toml
# scene_mapping：调用来源 → 场景组
[scene_mapping]
user_chat = "scene_admin"
proactive = "scene_normal"
script_free_dialogue = "scene_normal"
script_ai_dialogue = "scene_default"
entry_greeting = "scene_default"

# 场景组权限
[scene_groups.scene_admin]
enabled = true
all_tools = true                    # 管理员：允许所有工具（含未来新增）

[scene_groups.scene_normal]
enabled = true
tools = ["get_current_time"]        # 普通：显式列出（当前内置工具只有它）

[scene_groups.scene_default]
enabled = false                     # 默认：禁用（AI 剧本对话 / 入场问候没有工具）

# 角色组权限
[role_groups.default]
enabled = false                     # 新角色默认进 default 组，无工具权限
roles = []                          # initialize_characters 会把未归组角色填进来
```

> `scene_mapping` 键用 snake_case（`serde(rename_all = "snake_case")`）。

## 3. 默认语义

| 来源（GeneratorSource） | 场景组 | 结果 |
|---|---|---|
| `UserChat` 玩家发消息 | `scene_admin`（all_tools） | 可用全部工具（只要角色组放行） |
| `Proactive` 主动对话 | `scene_normal`（显式列表） | 可用列表内的工具 |
| `ScriptFreeDialogue` 剧本自由对话 | `scene_normal` | 同上 |
| `ScriptAiDialogue` 剧本 AI 对话 | `scene_default`（disabled） | **无任何工具** |
| `EntryGreeting` 入场问候 | `scene_default`（disabled） | **无任何工具** |

角色侧默认：

- **所有已知角色**（`Main` / `Npc`，来自 DB）在启动时被 `initialize_characters` 加入 `default` 角色组；
- **`default` 组 `enabled=false`** —— 换句话说，**默认情况下所有角色都没有工具权限**，必须显式把角色移进某个启用且有工具集的组，工具才真正生效；
- 角色与角色组是**一对一的**：`add_role_to_group` 会把角色从其它所有组移除；`default` 组不可删除。

## 4. 管理 API（Rust 侧）

`ToolPermissionConfig` 暴露的方法（均在 `permissions.rs`）：

**场景组**：`get_scene_group(name)` / `set_scene_group(name, ToolPermission)` / `delete_scene_group(name)`

**角色组**：`create_role_group(name, GroupPermission)` / `delete_role_group(name)`（`default` 禁删）/ `get_role_group_roles(name)` / `get_all_role_groups()`
`add_role_to_group(group, role)`（自动从其他组移除）/ `remove_role_from_group(group, role)` / `find_role_group(role)`

**查询**：`allowed_tools(source, role_name, all_names)`（核心，见下）

## 5. `allowed_tools` 计算逻辑

这是整个权限模型的入口，`ToolRegistry::allowed_tools` 每轮生成都调它一次，得到允许工具集，再用于裁剪下发给 LLM 的定义、并放进 `ToolContext` 供执行层二次校验。

```text
allowed_tools(source, role_name, all_names)
  ① key = source 转 GeneratorSourceKey
     场景组名 = scene_mapping.get(key) ?? "scene_default"     // 映射缺失回退默认
  ② 场景组不存在或 enabled=false            → 返回空集
  ③ role_name 为 None                       → 返回空集（未指定角色 = 无工具）
  ④ 该角色不在任何角色组                    → 返回空集
  ⑤ 角色组 enabled=false                    → 返回空集
  ⑥ 交集 / all_tools 短路：
     group.all_tools && (scene.all_tools || scene.tools 为空) → all_names（全集）
     scene.all_tools && (group.all_tools || group.tools 为空) → all_names（全集）
     其余 → scene.tools ∩ group.tools
```

设计要点：

- **`all_tools` 是双向短路**：任一方开 `all_tools`，另一方就给全量或列表；**双方都 `all_tools` 时返回全集**。这样「管理员场景组开 all_tools」时，即使角色组只有空列表也能拿到全部工具；
- **角色名是硬门槛**：`role_name = None`（比如还没选定主角）直接空集 —— 工具权限绑定在角色身份上，而不是绑定在调用来源上；
- 场景/角色任一侧 `enabled=false` 都直接清空 —— 默认 `scene_default` 与 `default` 角色组都是关闭的，形成双保险。

## 6. 调用链（一次生成里权限被用两次）

```text
run_pipeline
   ├─ stream_with_tool_loop
   │     allowed   = registry.allowed_tools(source, role_name)     // 第一次：算集合
   │     definitions = registry.definitions_for_allowed(&allowed)   // 裁剪定义 → 下发给 LLM
   │     context    = ToolContext::new(allowed)                      // 塞进执行上下文
   │     ...
   │     executor.execute(name, args, &context)
   │         ├─ context.allows(name) ?                              // 第二次：执行前复核
   │         └─ registry.get(name) → 执行
   └─ ...
```

两次校验的意义：**LLM 视角（下发定义）** 与 **执行视角（放行集合）** 使用同一个允许集合，杜绝「定义下发时过滤了，执行时却能调」或反之的口径不一致。

## 7. 常见操作示例

```toml
# 例1：让「诺一」在玩家对话里能用全部工具
# 角色归属到 admin 组（或用代码 add_role_to_group("admin","诺一")）
[role_groups.admin]
enabled = true
all_tools = true
roles = ["诺一"]
```

```toml
# 例2：剧本 AI 对话也要能用 get_current_time（默认 scene_default 是禁用的）
[scene_mapping]
script_ai_dialogue = "scene_story"        # 把剧本 AI 对话映射到新场景组

[scene_groups.scene_story]
enabled = true
tools = ["get_current_time"]
```

## 8. 注意事项

- **角色名 = `ai_name`（角色显示名）**：`initialize_characters` 收的是 `get_all_tool_role_names` 返回的 `ai_name`，生成管线查权也用 `display_name`（= `ai_name`）—— 改角色卡显示名会导致权限归属失效，需要重新 `add_role_to_group`；
- **新工具不会自动进显式列表**：`all_tools: true` 的组自动包含新工具；`tools = [...]` 显式列表的组需要手动补（见 [extension.md](extension.md) §4）；
- **权限是「先算集合、后裁剪」**：每次生成都现算，没有缓存 —— 改完 TOML 即时生效（不必重启），代价是每轮一点哈希计算。
