# LingChat 插件开发指南

## 目录结构

一个插件是一个目录，放在 `data/plugins/<id>/`：

```
data/plugins/<id>/
├── manifest.toml   # 插件声明（必须）
└── <脚本文件>.py    # 工具处理脚本（manifest 里声明）
```

## manifest.toml

这里有一个示例

```toml
id = "tavily"              # 必须与目录名一致
name = "Tavily 搜索"
description = "基于 Tavily 的联网搜索与网页提取"
version = "0.1.0"
author = "LingChat"

# 可选：设置页渲染配置表单。kind 支持 string / secret / number / boolean
[[config]]
key = "max_results"
label = "默认返回条数"
kind = "number"
required = false

# 可选：环境变量白名单。宿主只把这里声明的变量注入 ctx.env，插件读不到其他环境变量
[[env]]
key = "TAVILY_API_KEY"
label = "Tavily API Key（来自进程环境）"

# 一个插件可声明多个工具，共用一个或多个脚本
[[tools]]
name = "tavily_search"     # 注册进 ToolRegistry，需全局唯一，建议带插件前缀
description = "联网搜索，返回相关网页摘要与链接"   # 不用说了吧，介绍
timeout_ms = 30000         # 单次执行超时（毫秒），默认 30000，上限 120000
script = "tavily.py"
# JSON Schema，必须是合法的 JSON 字符串，描述 LLM 需要的参数
parameters = '{ "type":"object", "properties":{ "query":{"type":"string"}, "max_results":{"type":"integer","default":5} }, "required":["query"] }'
```

## 脚本结构

每个工具执行时会都新建一个 Python 解释器，读取脚本，执行顶层定义后调用你定义的 `run(ctx)`：

```python
def run(ctx):
    tool = ctx["tool_name"]
    if tool == "my_search":
        return {"ok": True, "results": [...]}
    return {"ok": False, "error": "unknown_tool"}
```

- `run(ctx)` 必须返回可 JSON 序列化的 dict（`Value`）。
- 顶层定义（import、函数、常量）在**沙箱拦截之前**执行；`import os/subprocess/shutil/pathlib/ctypes/sysconfig` 会直接抛 `ImportError`，这些模块被置为不可用。
- 每次工具调用都会**新建**解释器，脚本里的全局状态不跨调用保留。

### ctx 注入的字段

| 字段               | 类型     | 说明                                               |
| ------------------ | -------- | -------------------------------------------------- |
| `ctx["tool_name"]` | str      | 当前被调用的工具名                                 |
| `ctx["args"]`      | dict     | 本次调用的参数（LLM 或调用方传入，按 schema 校验） |
| `ctx["config"]`    | dict     | 插件配置（设置页表单保存的值）                     |
| `ctx["env"]`       | dict     | 白名单环境变量，`ctx["env"].get("KEY")`            |
| `ctx["call_tool"]` | function | 调用任意已注册工具，见下文                         |

## 调用内置工具：`ctx["call_tool"]`

插件脚本可以调用 **所有已注册的 LLM tools**（目前是内置 15 个 + 其他插件注册的），返回该工具产出的 JSON dict：

```python
def run(ctx):
    call_tool = ctx["call_tool"]
    status = call_tool("status_get_current", {})
    todos = call_tool("schedule_get_all", {})
    notes = call_tool("memory_get_notes", {"role": "莱姆"})
    return {"ok": True, "now_role": status.get("current_role_id")}
```

- 第一个参数是工具名（str），第二个是参数 dict（无参数传 `{}`）。
- 成功返回工具结果的 JSON dict；工具执行失败或超时抛 `ValueError`，脚本可用 `try/except` 捕获。
- **注意**：`call_tool` 可以调用含写操作的工具（`memory_add_note`、`schedule_add_todo`、`scene_switch`、`character_switch` 等），且当前沙箱不校验调用方身份——安装第三方插件前请自行评估。

## HTTP 请求：`from plugin_host import http_get, http_post`

这里特别讲一下，除了可以使用标准库，我们还提供了复用项目的 reqwest 客户端的方法（webpki-roots，Android 兼容）：

```python
from plugin_host import http_get, http_post

# GET
r = http_get("https://example.com/api", query={"q": "news"}, headers={"X-Key": "v"}, timeout_ms=30000)

# POST（body 自动 JSON 序列化）
r = http_post("https://example.com/api", headers={"Authorization": "Bearer xx"}, body={"query": "x"})

# 返回统一结构：
# { "status": 200, "ok": true, "body": <解析后的 JSON 或字符串> }
# 请求失败时: { "ok": false, "error": "..." }
```

注意 `body` 里的 JSON 在 `r["body"]` 字段下，不是顶层（打个比方，Tavily 结果要取 `r["body"]["results"]`）。

## 内置工具 API 清单

以下 15 个工具可直接通过 `call_tool(name, args)` 调用。

### 时间

**`get_current_time`**

- 参数：`{}`
- 返回：`{ local_time: string, timezone: string, unix_timestamp: number }`

### 日程（读写 `game_data/schedules.json`）

**`schedule_get_all`**

- 参数：`{}`
- 返回：完整日程配置（`todo_groups`、重要日子等，序列化自 `UserScheduleSettings`）

**`schedule_add_todo`**

- 参数：`{ text: string(必), group?: string, priority?: number, deadline?: string }`
- 返回：`{ ok: true, id: number }`

**`schedule_update_todo`**

- 参数：`{ id: number(必), done?: boolean, text?: string, priority?: number }`（至少一项）
- 返回：`{ ok: true, id: number }`

**`schedule_delete_todo`**

- 参数：`{ id: number(必) }`
- 返回：`{ ok: true, id: number }`

### 记忆（角色笔记文件 + 自动记忆库）

**`memory_get_current`**

- 参数：`{}`
- 返回：`{ role_id: number, memory: string }`（当前角色的自动记忆库文本）

**`memory_get_notes`**

- 参数：`{ role?: string }`（不传读当前角色；传其他角色名只读）
- 返回：`[ { id: string, content: string, tags: string[], created_at: string } ]`

**`memory_add_note`**

- 参数：`{ content: string(必), tags?: string[] }`（仅写当前角色）
- 返回：`{ ok: true, id: string }`

**`memory_update_note`**

- 参数：`{ id: string(必), content?: string, tags?: string[] }`（至少一项）
- 返回：`{ ok: true, id: string }`

**`memory_delete_note`**

- 参数：`{ id: string(必) }`
- 返回：`{ ok: true, id: string }`

### 状态（读运行时 `game_status`）

**`status_get_current`**

- 参数：`{}`
- 返回：`{ player, current_role_id, onstage_role_ids, present_role_ids, main_role_id, background, present_pic, background_music, background_effect, current_scene_id, scene_awareness_enabled, global_variables }`

**`status_get_scene`**

- 参数：`{}`
- 返回：`{ current_scene_id: string, name: string, description: string, background: string }`

### 场景（读写 SceneStore）

**`scene_list`**

- 参数：`{}`
- 返回：`[ { id: string, name: string, description: string, background: string } ]`

**`scene_switch`**

- 参数：`{ id?: string, name?: string }`（提供其一）
- 返回：`{ ok: true, scene_id: string }`

### 角色（读写数据库 + game_status）

**`character_list`**

- 参数：`{}`
- 返回：`[ { id: number, name: string } ]`

**`character_switch`**

- 参数：`{ id: number(必) }`
- 返回：`{ ok: true, role_id: number }`

## 完整示例

一个「查询并汇报当前状态」的插件：

```toml
# data/plugins/my_status/manifest.toml
id = "my_status"
name = "状态汇报"
description = "查询当前角色状态并简要汇报"
version = "0.1.0"
author = "LingChat"

[[tools]]
name = "my_status_report"
description = "查询当前角色的状态并返回摘要"
timeout_ms = 5000
script = "main.py"
parameters = '{ "type":"object", "properties":{}, "required":[] }'
```

```python
# data/plugins/my_status/main.py
def run(ctx):
    call_tool = ctx["call_tool"]
    status = call_tool("status_get_current", {})
    scene = call_tool("status_get_scene", {})
    return {
        "ok": True,
        "player": status.get("player"),
        "current_role_id": status.get("current_role_id"),
        "scene": scene.get("name"),
        "scene_description": scene.get("description"),
    }
```

> 当然你不用写这种整合插件，系统自己就有滴💦

## 沙箱与限制

以下都是为了用户安全考虑，当然不是没有跳过沙箱的方法，但是最好还是别用哦（不然禁止上传官方创意工坊）

- 禁用的顶层模块：`os`、`subprocess`、`shutil`、`pathlib`、`ctypes`、`sysconfig`
- 环境变量只有 manifest `[[env]]` 白名单内的会注入 `ctx["env"]`
- 脚本无法直接读写文件系统、启动子进程、加载系统库。
- 每次调用新建解释器，无跨调用状态；超时（`timeout_ms`，上限 120000ms）后执行结果作废、本次调用终止。
- **注意**：超时无法强制中断脚本所在的阻塞线程，死循环可能残留占用线程直至进程退出，插件作者（和你们的agent）应避免写死循环。
- `call_tool` 是有意的受信任通道，可触达所有注册工具（含写操作）。（谨慎使用）
