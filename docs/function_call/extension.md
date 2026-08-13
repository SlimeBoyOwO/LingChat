# 如何扩展新的工具函数（extension）

> 扩展步骤图见 [diagrams/extend.html](diagrams/extend.html)。

## 1. 总览：一个工具 = 实现一个 trait + 注册 + 配置权限

在 PR #523 的架构下，新增一个聊天工具只需要三步：

```text
① 实现 Tool trait（定义 + 执行）      → 新文件，如 tools/my_tool.rs
② 在 built_in_registry 里注册          → tools/mod.rs 一行
③ 给角色/场景配置权限                  → 编辑 tool_permissions.toml（或用权限 API）
```

不需要动 LLM 层、不动权限核心、不动台词表与记忆 —— 它们对工具都是透明的。

## 2. 第一步：实现 `Tool` trait

把 `src-tauri/src/ai_service/tools/clock.rs` 当模板（它就是内置示例工具 `get_current_time`）：

```rust
use async_trait::async_trait;
use serde_json::Value;
use crate::ai_service::types::ToolDefinition;
use super::executor::{Tool, ToolContext, ToolError, ToolResult};

pub struct MyTool;                                   // 通常是零大小的 unit struct

#[async_trait]
impl Tool for MyTool {
    /// 给 LLM 看的定义：OpenAI function calling 格式
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "my_tool",                               // 工具名（全局唯一）
            "工具的用途描述，越具体 LLM 越会用对",     // description
            serde_json::json!({                      // JSON Schema 参数
                "type": "object",
                "properties": {
                    "keyword": {
                        "type": "string",
                        "description": "要查的关键词"
                    },
                    "limit": { "type": "integer", "default": 10 }
                },
                "required": ["keyword"],             // 必填参数
                "additionalProperties": false        // 拒绝多余参数（clock.rs 的测试就验这个）
            }),
        )
    }

    /// 执行：入参是解析好的 JSON object，返回 JSON 结果
    async fn execute(&self, context: &ToolContext, arguments: Value) -> Result<ToolResult, ToolError> {
        // 1) 入参必须是 JSON object
        let Some(args) = arguments.as_object() else {
            return Err(ToolError::InvalidArguments("参数必须是 JSON object".into()));
        };

        // 2) （可选）二次校验权限 —— 执行层已拦过，这里兜底
        // if !context.allows("my_tool") { return Err(ToolError::Execution("无权限".into())); }

        // 3) 读参数
        let keyword = args.get("keyword")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("缺少 keyword".into()))?;

        // 4) 干活……
        // 5) 返回 JSON（ToolResult = serde_json::Value）
        Ok(serde_json::json!({ "result": format!("查到了 {keyword}") }))
    }
}
```

### trait 签名要求

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn definition(&self) -> ToolDefinition;
    async fn execute(&self, context: &ToolContext, arguments: Value)
        -> Result<ToolResult, ToolError>;
}
```

- 必须 `Send + Sync`：注册后放进 `Arc<dyn Tool>`，被多个并发生成任务共享；
- `definition()` 同步返回，每次下发前都会被调用（所以描述要稳定，不要依赖可变状态）；
- `execute()` 是异步的，可以自由 `await` 任何东西（HTTP、DB、文件），但**有 2 秒超时** —— 慢操作要么自己分页/截断，要么返回「需要继续」之类让调用方知晓。

## 3. 第二步：注册

在 `tools/mod.rs::built_in_registry` 里加一行：

```rust
pub fn built_in_registry(role_names: ...) -> Result<ToolRegistry> {
    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(CurrentTimeTool))?;
    registry.register(Arc::new(MyTool))?;               // ← 新增
    // 权限配置随注册自动带出工具名
    let mut permissions = ToolPermissionConfig::load_or_create(...)?;
    ...
}
```

注意点：

- **工具名全局唯一**：`register` 对重名直接返回 `RegistryError::DuplicateName`，启动即失败 —— 所以改名要改 `definition().function.name`，同时检查旧名字没有在 `tool_permissions.toml` 里被引用；
- **注册顺序即下发顺序**：`definitions()` 按注册顺序返回，LLM 看到的工具列表顺序稳定。把最常用/最该先试的工具放前面；
- 工具名用 `snake_case`（如 `get_current_time`），与各家 API 的 function calling 约定一致。

## 4. 第三步：配置权限

新工具默认情况取决于它注册后落在哪个权限组合里（见 [permission.md](permission.md)）：

- **`default` 角色组默认 `enabled=false`** —— 新角色、以及没显式加入任何组的角色，**拿不到任何工具**；
- 想让某个角色能用你的工具：把它加进一个有权限的角色组（`scene_admin`/`scene_normal` 映射到的场景组默认允许**全部工具**，若你的工具名在它们 `tools` 集合里）；
- 权限文件 `<data_dir>/tool_permissions.toml` 是**按工具名枚举**的，`all_tools: true` 的组自动包含新增工具，**显式 `tools = [...]` 的组不会** —— 新增工具后要检查显式列表是否要补。

示例：把角色「诺一」放进 `devs` 组并只给它 `get_current_time` 和 `my_tool`：

```toml
[role_groups.devs]
enabled = true
tools = ["get_current_time", "my_tool"]
roles = ["诺一"]

[scene_mapping]          # 让 UserChat 场景用 scene_admin（默认已是）
```

若暂时不想碰 TOML，也可以用 Rust 侧的权限 API（`ToolPermissionConfig::set_scene_group` / `create_role_group` / `add_role_to_group` / `allowed_tools`）在代码里程序化配置。

## 5. 错误处理规范

**工具执行失败不能 `panic` / 不能向上抛 `anyhow`** —— `ToolExecutor` 只认识 `ToolError`，其他错误会走 `tool_error` 兜底。正确姿势：

| 情况 | 做法 |
|---|---|
| 参数不合法 | `return Err(ToolError::InvalidArguments("说明".into()))` |
| 执行逻辑失败 | `return Err(ToolError::Execution("说明".into()))` |
| 需要让 LLM 看到的结构化失败 | 返回一个 `{ok:false, reason:...}` 的 JSON 结果（`Ok` 分支） |

所有失败都会被 `ToolExecutor` 统一编码成：

```json
{ "ok": false, "error": { "code": "tool_error", "message": "说明" } }
```

并经 `LlmMessage::tool_result` 回填给 LLM。**这是特性不是 bug**：模型会读到「工具失败了 + 原因」，然后自行决定换个参数重试或改问别的方式。

## 6. 参数校验规范

- 入参 `arguments` 是**字符串化 JSON**，`ToolExecutor` 已帮你 `serde_json::from_str` 成 `Value`，并保证是 `Object`（否则直接 `invalid_arguments`）；
- 你自己的 `execute` 仍要校验字段：类型不匹配、缺必填、超范围 —— 统一返回 `ToolError::InvalidArguments`，**不要 panic**；
- 在 `definition().parameters` 里用 `additionalProperties: false` + 清晰的 `description`，让模型在源头就少传错参数（`clock.rs` 的测试专门验证了这一点）。

## 7. 测试建议

参考 `clock.rs` / `executor.rs` 自带的内嵌测试：

- **definition 测试**：`additionalProperties` 是否为 `false`、工具名是否符合预期；
- **execute 成功路径**：真实执行一次，断言返回 JSON 的字段；
- **execute 失败路径**：传非法参数，断言返回 `ToolError::InvalidArguments`；
- **executor 集成**：注册假工具后走 `ToolExecutor`，验证 `unknown_tool` / `invalid_json` / `timeout`（用 `ToolExecutor::with_timeout` 把超时调短）等稳定错误码；
- **registry 测试**：重名注册返回 `DuplicateName`，`get` 找不到返回 `None`。

工具执行可能引入外部依赖（网络 / 文件），测试里用「假数据工具」（如 `executor.rs` 测试里的 `EchoTool` / `SlowTool`）隔离，不要打真实外部服务。

## 8. 常见问题

- **LLM 看不到我的工具** → 检查三处：① 注册了吗（`registry.definitions()` 有没有）；② Provider 支持流式工具吗（只有 genai 系走工具循环，Kimi Code 聊天路径不下发工具）；③ 权限里放行了吗（`allowed_tools` 非空，且 `definitions_for` 返回了它）。
- **工具被调用了但没有结果写进对话** → 检查 `run_pipeline` 的 ③ 是否执行（`tool_messages` 非空才会落表）；工具结果行是 `LineAttribute::Tool`，玩家看不见是正常现象。
- **下次生成模型忘了刚查的东西** → 工具历史已经写进台词表并重建记忆，若仍不生效，检查是否手动改过 `line_list` 后没调 `refresh_memories`。
- **改工具名后启动报重名/找不到** → `tool_permissions.toml` 里残留旧名不影响启动（`load_or_create` 按新工具名重建默认，但显式列表里旧名会导致该组放行失效），建议清理。
