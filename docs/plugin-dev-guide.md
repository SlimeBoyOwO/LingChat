# LingChat 插件开发指南

## 目录结构

一个插件是一个目录，放在 `data/plugins/<id>/`：

```
data/plugins/<id>/
├── manifest.toml   # 插件声明（必须）
├── <脚本文件>.py    # 工具处理脚本（manifest 里声明）
├── plugin_host.pyi # 可选：类型存根，仅供编辑器补全，不参与运行（见下）
└── 资源子目录（可选）# 见「插件携带资源」：characters / scripts / musics / backgrounds / ambients
```

## 编辑器补全：`plugin_host.pyi`

`plugin_host` 是宿主注入进解释器的原生模块（源码在 `crates/ling-chat-plugins/src/host_api.rs`），磁盘上并没有这个包，所以 `from plugin_host import http_post` 在编辑器里会被标成「找不到模块 / 无法解析导入」。**这不影响运行**，只是缺补全和类型检查。

仓库里的 `docs/plugin_host.pyi` 就是它的类型存根（覆盖全部宿主函数）。把这份文件拷进插件目录（与 `.py` 脚本同级），报错即消失并恢复补全：

```
data/plugins/<id>/
├── manifest.toml
├── main.py
└── plugin_host.pyi   # 类型检查器读取；运行时不加载，可安全随插件一起分发
```

存根靠「与脚本同级」被解析——把插件目录本身作为 IDE 工程根打开即可命中。若工程根在别处，把存根所在目录加进 Pylance 的 `extraPaths`（PyCharm 用「Sources Root」）同样有效。

## 打包与导入

除了手工把目录放进 `data/plugins/<id>/`，也可以在 **设置 → 插件 → 从压缩包导入插件** 直接导入 `.zip` / `.7z`。压缩包只接受两种结构：

```
plugin.zip                  plugin.zip
├── manifest.toml           └── my-plugin/     ← 只允许套这一层
├── tavily.py                   ├── manifest.toml
└── characters/...              ├── tavily.py
    （形式 A：根即插件）          └── characters/...
                                    （形式 B：套一层文件夹）
```

其余结构一律判为非法并中止导入（不落盘、不清空同名插件），例如：`manifest.toml` 埋在两层文件夹以下、压缩包内并列多个插件目录、根目录既有散落文件（`README.md` 之类）又套了一层插件文件夹。

导入时的硬性校验：

- 必须有 `manifest.toml`，且能被后端按现有 schema 解析（缺字段、多未知字段、TOML 语法错都算失败）
- `id` 只允许字母、数字、`_`、`-`（它同时是**落地目录名**，见下）
- `[[tools]]` 声明的每个 `script` 必须在包里真实存在
- `resources` 声明了某类资源但对应子目录不存在 → **允许**（视作该插件这次没带这类资源）

两点与角色导入不同的行为值得注意：

1. **目录名取 `manifest.id`，不取压缩包文件名**。因为后端强制 `manifest.id == 目录名`，改名会让插件加载失败，所以同名冲突只有「覆盖」与「放弃」两种选择，没有自动改名。
2. **导入后插件默认是关闭的**，需要在插件页手动开启（压缩包会跑沙箱脚本、也会往角色/剧本/场景里注入资源）。选「覆盖」时旧插件连同它的配置与启用状态一起被清除，等同于全新安装。

## manifest.toml

这里有一个示例

```toml
id = "tavily"              # 必须与目录名一致
name = "Tavily 搜索"
description = "基于 Tavily 的联网搜索与网页提取"
version = "0.1.0"
author = "LingChat"

# 可选：允许 read_data_file 读取的 data/ 下相对路径（目录前缀，含其子树）。
# 未声明的路径一律拒绝；不写 = 不能读任何文件。
read = ["game_data/characters", "voice"]

# 可选：设置页渲染配置表单。kind 支持 string / secret / number / boolean
[[config]]
key = "max_results"
label = "默认返回条数"
kind = "number"
required = false

# hint 可选：会以一行小字显示在输入框下面，告诉用户该填什么、去哪儿拿。
# \n 可以换行（比如一行写本机、一行写服务器）。
[[config]]
key = "endpoint"
label = "接收地址"
kind = "string"
required = true
default = "http://127.0.0.1:8080/reply"
hint = "本机就用默认值；对接别的机器时，把 127.0.0.1 换成对方的地址。"

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

除工具与资源，插件还可以订阅宿主信号（`[[subscribe]]`）、声明 WebSocket 连接（`[[ws]]` 与 `ws_allow`）、声明启动入口（`[startup]`）、前置插件（`depends_on`）与可读素材范围（`read`），见后文对应章节。

## 插件携带资源（人物 / 剧本 / 音乐 / 背景图 / 环境音）

除了工具，插件还可以携带内容资源，直接混入游戏对应列表并带「插件」来源角标。
在 `manifest.toml` 顶层声明支持哪些资源类型（`resources` 数组，可省略；省略即纯工具插件）：

```toml
resources = ["characters", "scripts", "musics", "backgrounds", "ambients"]
```

资源目录与游戏目录**同名同构**，放在插件目录下，运行时**直读、不复制**：

```
data/plugins/<id>/
├── characters/<角色文件夹>/{settings.yml, avatar/头像.webp, avatar/<情绪>.webp, ...}
├── scripts/…               # 与 game_data/scripts 相同三种布局（character/ standalone/ 平铺）
├── musics/*.mp3
├── backgrounds/*.webp
└── ambients/*.mp3
```

- 角色文件夹内部结构与 `game_data/characters/` 完全一致；剧本内部结构与 `game_data/scripts/` 一致
  （资源只认剧本自己的 `Assets/`，插件根目录不参与全局兜底）。
- 一个插件可以**没有工具、只有资源**（此时 `[[tools]]` 可整体省略）。

### 冲突与玩家操作

- **同名冲突**：插件资源与游戏现有资源同名（角色按文件夹名、剧本按 `script_name`、图/音按文件名）
  → 默认使用游戏版，插件版隐藏，插件管理页标「冲突」。插件之间同名 → 先注册者（id 序）赢。
- 玩家在「设置 · 插件」的资源区可对单条资源：**隐藏**（软删除，列表不再显示、文件保留）、**恢复**、
  **保留**（复制到游戏目录成为游戏自有资源，复制后插件版自动隐藏）。
- 删除插件只删插件本体，已「保留」到游戏目录的资源不受影响。
- 插件人物会入库，可像普通角色一样完整对话、换装、加入场景；插件被禁用 / 删除时其角色自动从列表移除。

## 脚本结构

每次执行都会新建一个 Python 解释器，读取脚本，执行顶层定义后调用入口函数——工具走 `run(ctx)`，信号订阅走 `[[subscribe]]` 声明的 `handler(ctx)`（见「订阅宿主信号」）。两类入口可以放在同一个脚本里：

```python
def run(ctx):
    tool = ctx["tool_name"]
    if tool == "my_search":
        return {"ok": True, "results": [...]}
    return {"ok": False, "error": "unknown_tool"}
```

- `run(ctx)` 必须返回可 JSON 序列化的 dict（`Value`）。
- 顶层定义（import、函数、常量）在**沙箱拦截之前**执行；`import os/subprocess/shutil/pathlib/ctypes/sysconfig` 会直接抛 `ImportError`，这些模块被置为不可用。
- 每次执行都会**新建**解释器，脚本里的全局状态不跨调用保留。

### ctx 注入的字段（工具）

| 字段               | 类型     | 说明                                                                |
| ------------------ | -------- | ------------------------------------------------------------------- |
| `ctx["tool_name"]` | str      | 当前被调用的工具名                                                  |
| `ctx["args"]`      | dict     | 本次调用的参数（已按 schema 校验，未知字段被拒）；无参工具为空 dict |
| `ctx["config"]`    | dict     | 设置页保存的配置；未填的字段不出现，`number` 一律为 float           |
| `ctx["env"]`       | dict     | 白名单环境变量（`[[env]]` 声明且进程里存在），用 `.get("KEY")` 读   |
| `ctx["call_tool"]` | function | 调用任意已注册工具，见下文                                          |

## 调用内置工具：`ctx["call_tool"]`

插件脚本可以调用 **所有已注册的 LLM 工具**（内置工具 + 其他插件注册的工具），返回该工具产出的 JSON dict：

```python
def run(ctx):
    call_tool = ctx["call_tool"]
    status = call_tool("status_get_current", {})
    todos = call_tool("schedule_get_all", {})
    notes = call_tool("memory_get_notes", {"role": "莱姆"})
    return {"ok": True, "now_role": status.get("current_role_id")}
```

入参：第一个是工具名（str，**区分大小写**），第二个是参数 dict（无参工具传 `{}`）。

返回：统一 JSON dict——成功 `{ "ok": true, ... }`，失败 `{ "ok": false, "error": { "code", "message" } }`。未知工具、参数非法、执行超时等都走失败分支，**不再抛异常**，脚本先判 `r.get("ok")` 即可。

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

## 读游戏素材：`from plugin_host import read_data_file`

插件的沙箱不允许直接读文件系统，但有些插件确实需要游戏自己的素材——比如把角色立绘
裁成表情包发给外部服务、或者把 TTS 语音转发出去。`read_data_file` 就是给这个用的：

```toml
# manifest.toml：先声明能读哪些目录（相对 data/ 的目录前缀，含其子树）
read = ["game_data/characters", "voice"]
```

```python
from plugin_host import read_data_file

r = read_data_file("game_data/characters/风雪/avatar/高兴.webp")

# 成功：{ "ok": true, "size": 12345, "base64": "..." }
# 失败：{ "ok": false, "error": "..." }
```

- **必须先声明**：路径要落在 manifest `read` 声明的某个前缀之下，没声明的目录读不到
  （返回 `ok: false`，`error` 里会提示「未声明」）；不写 `read` = 一个文件都读不了
- `read` 里只能写**相对 `data/`** 的路径，`..`、绝对路径、盘符会让 manifest 直接校验失败
- 请求路径同样只接受相对 `data/` 的路径：`..`、绝对路径、以及指向声明目录外的软链接都会被拒绝
- 单个文件上限 64MB，超了返回 `ok: false`
- 失败不抛异常，按返回值处理即可；`error` 里只有你自己给的相对路径，不会带宿主绝对路径
- 目录名和角色显示名不一定一样（立绘目录由角色数据决定），插件侧别按显示名硬拼

## 订阅宿主信号：`[[subscribe]]`

工具是「LLM 来调你」，信号是「宿主来调你」。在 manifest 顶层声明要监听哪些信号：

```toml
[[subscribe]]
signal = "ai_reply"              # 宿主注册的信号名，见下表
script = "hook.py"               # 处理脚本（相对插件目录的单个文件名）
handler = "on_reply"             # 脚本内的处理函数，签名为 handler(ctx)
timeout_ms = 30000               # 可选，单次执行超时（默认 30000，上限 120000）
match = { emotion = "高兴" }      # 可选，见下
```

### 已注册的信号

| 信号         | 触发时机                                               | payload                                              |
| ------------ | ------------------------------------------------------ | ---------------------------------------------------- |
| `ai_reply`   | 每条助手回复。自由对话、剧本固定台词、主动消息都会触发 | 与前端 `ai:reply` 事件一致（camelCase），见下        |
| `ws_message` | 插件声明的 WS 连接：建立 / 断开 / 收到帧 / 出错        | `{ connId, mode, event, data, binary, error }`，见下 |

`ai_reply` 的 payload 顶层字段：`type`、`duration`、`isFinal`、`character`、`roleId`、`emotion`、`originalTag`、`message`、`ttsText`、`motionText`、`audioFile`、`originalMessage`、`displayName`、`displaySubtitle`、`userMessageSeq`、`thinking`、`previewGen`。

比前端收到的 `ai:reply` 事件多一个 `avatarDir`：角色的立绘目录（相对 `data/`），
例如 `game_data/characters/风雪/avatar`。角色的显示名和目录名不一定一样，所以由宿主查库给出。该键**始终存在**：没有立绘目录（剧本 / 插件角色等）时为 `null`，可直接下标取值。

`ws_message` 的 payload 顶层字段：

| 字段     | 类型 | 说明                                                                     |
| -------- | ---- | ------------------------------------------------------------------------ |
| `connId` | str  | 连接名（`[[ws]]` 的 `id`）                                               |
| `mode`   | str  | 连接方向：`"client"` / `"server"`                                        |
| `event`  | str  | `"connect"` / `"disconnect"` / `"message"` / `"error"`                   |
| `data`   | any  | 仅 `message`：文本帧尽量解析成 JSON（失败给原字符串）；二进制帧给 base64 |
| `binary` | bool | `data` 是否为二进制帧的 base64                                           |
| `error`  | str? | 仅 `error`：失败原因                                                     |

> `ws_message` 只派发给**声明了该连接的插件**——别的插件就算订阅了同名信号也收不到。
> 连接本身用 `[[ws]]` 声明、用 `ws_open` / `ws_send` / `ws_close` 操作，见「WebSocket 连接」。`ws_message` 的 handler `ctx` 与 `ai_reply` 完全一致（`ctx["signal"]` 为 `"ws_message"`）。

> 声明**未注册**的信号是安全的：加载时只对未注册的信号打一条 warn，不算 manifest 错误，插件包在信号上线前后都能正常安装启用。
>
> 插件也可以**只有订阅、没有工具和资源**。

### 匹配：`match`

`match` 在**宿主侧**筛选，键是信号 payload 的顶层字段名：

- 标量 = 等值命中：`match = { emotion = "高兴" }`
- 数组 = 命中其中任一：`match = { emotion = ["高兴", "害羞"] }`
- 省略 `match` = 一律派发
- payload 里没有该字段 = **不命中**（不会退化成通配）
- 多个键之间是「与」

判断逻辑刻意做得便宜，为的是在派发前把绝大多数信号挡掉——只有命中的订阅才会新建解释器。复杂条件（正则、跨字段比较、依赖运行时状态）请在 handler 内部自己判。

### handler 的 `ctx`

与工具脚本形状一致，只把 `tool_name` / `args` 换成 `signal` / `payload`：

| 字段               | 类型     | 说明                                               |
| ------------------ | -------- | -------------------------------------------------- |
| `ctx["signal"]`    | str      | 触发的信号名                                       |
| `ctx["payload"]`   | dict     | 信号载荷；`match` 就是在这个 dict 的顶层字段上筛选 |
| `ctx["config"]`    | dict     | 同工具脚本                                         |
| `ctx["env"]`       | dict     | 同工具脚本                                         |
| `ctx["call_tool"]` | function | 同工具脚本                                         |

```python
# data/plugins/my_plugin/hook.py
from plugin_host import http_post


def on_reply(ctx):
    payload = ctx["payload"]
    # 例：把每条回复推给外部服务（配置项在 manifest 的 [[config]] 里声明）
    http_post(
        ctx["config"]["endpoint"],
        body={"text": payload.get("message"), "emotion": payload.get("emotion")},
    )
```

约定与限制：

- **返回值会被丢弃**。信号没有下游消费者，不要靠 `return` 传递信息；想让结果进入对话，自己在 handler 里调 `call_tool`（如 `memory_add_note`）。
- **handler 必须尽快返回**。超时（`timeout_ms`）会放弃本次执行，但中断不了阻塞线程，死循环会一直占着那个线程。
- **同时执行的 handler 有上限**（当前 4 个）。达到上限时本次派发被**跳过**并记一条 warn，不排队。
- **每次派发都新建解释器**，全局状态不跨派发保留，和工具调用一样。
- **沙箱规则与工具脚本完全一致**（见文末「沙箱与限制」）。
- 插件被禁用 / 删除后订阅立即失效；重新启用后恢复。

## WebSocket 连接：`[[ws]]`

插件可以声明 WebSocket 连接，与外部程序双向通信。**连接本体常驻宿主**（插件脚本跑完即弃，长连接没法活在插件里），插件只做两件事：**收到帧时被叫醒执行 handler**（`ws_message` 信号）、**主动发帧**（`ws_send`）。

```toml
# 顶层：允许 client 连接的 URL 白名单。省略 = 允许全网段（默认）。
# 每项是前缀，可含 * 通配；"*" 等价全网段。
ws_allow = ["wss://example.com/*", "wss://*.mycorp.com/*"]

# client：插件连出去（连外部服务）
[[ws]]
id = "gateway"                 # 连接名，脚本按它 send/open/close；同一插件内唯一
mode = "client"
url = "wss://example.com/ws"   # 可含 ${config.键名} / ${env.变量名} 占位符；缺省则靠 ws_open(url=...) 传
auto_reconnect = true          # 可选，默认 true：断开后宿主带退避自动重连

# server：宿主监听，被外部连进来
[[ws]]
id = "remote"
mode = "server"
bind = "127.0.0.1:8787"        # 监听地址（必填）
path = "/ws"                   # 可选，默认 /ws
```

字段一览：

| 字段               | 适用   | 说明                                                                     |
| ------------------ | ------ | ------------------------------------------------------------------------ |
| `id`               | 都要   | 连接名，脚本按它操作；同一插件内唯一                                     |
| `mode`             | 都要   | `"client"`（连出去）/ `"server"`（被连进来）                             |
| `url`              | client | 默认连接的 URL（支持占位符）；缺省则由脚本 `ws_open(id, url=...)` 给出   |
| `headers`          | client | 可选，连接时的请求头（值支持占位符），如 `{ Authorization = "Bot xxx" }` |
| `auto_reconnect`   | client | 可选，默认 `true`                                                        |
| `bind`             | server | 监听地址，必填                                                           |
| `path`             | server | 可选，WebSocket 路径，默认 `/ws`                                         |
| `script`/`handler` | 都可   | 可选，内联事件处理脚本（等价于一条 `ws_message` 订阅，见下）             |
| `timeout_ms`       | 都可   | 可选，内联 handler 的超时，默认 30000                                    |

### 占位符

`url` 与 `headers` 的值里可以写 `${config.键名}` / `${env.变量名}`，宿主在建连时替换成运行期的配置值 / 白名单环境变量。适合放 token 等敏感信息（写进 `[[config]]` 的 `secret` 字段，别硬编码）。未命中的占位符原样保留。

```toml
[[ws]]
id = "gateway"
mode = "client"
url = "wss://gateway.example.com/ws?token=${config.bot_token}"
```

```toml
[[config]]
key = "bot_token"
label = "机器人 Token"
kind = "secret"
required = true
```

### 收到消息：handler 与 `ws_message`

连接建立 / 断开 / 收到帧 / 出错都会以 `ws_message` 信号派发（payload 见「订阅宿主信号」一节）。两种写法：

**写法一：内联 handler**（推荐，连接和处理放一处）

```toml
[[ws]]
id = "gateway"
mode = "client"
url = "wss://example.com/ws"
script = "ws_handler.py"
handler = "on_ws"
```

```python
# data/plugins/my_plugin/ws_handler.py
def on_ws(ctx):
    p = ctx["payload"]
    if p["event"] == "message" and p["connId"] == "gateway":
        print("收到:", p["data"])
```

**写法二：常规 `[[subscribe]]`**（用 `match` 只收某个连接）

```toml
[[subscribe]]
signal = "ws_message"
script = "hook.py"
handler = "on_ws"
match = { connId = "gateway" }
```

两种写法等价，`ctx` 形状完全一样。

### 发出消息：`ws_send`

在任意脚本里（工具、信号 handler、启动入口都行）调 `plugin_host.ws_send`（见「插件系统的私有 API」）：

```python
from plugin_host import ws_send
ws_send("gateway", {"op": 1, "d": {"hello": "world"}})   # dict → JSON 文本帧
ws_send("gateway", "plain text")                          # str → 文本帧
```

`ws_send` **同步发出、不等回执**。要拿对端响应，就等下一次 `ws_message` 事件（`event == "message"`）。

### 生命周期与脚本控制

- 插件**启用时**，宿主自动建立它声明的全部连接；**禁用 / 删除 / 重载**时全部断开，不留孤儿连接。
- 脚本可用 `ws_open(conn_id)` / `ws_close(conn_id)` 按需启停（例如先等配置就绪再连、或临时断开），用 `ws_status` 读当前状态。
- client 断开后由宿主自动重连（`auto_reconnect = false` 可关掉）。
- server 监听失败（端口被占等）以 `ws_message` 的 `error` 事件通知插件。

> server 模式会在本机监听端口。写 `0.0.0.0` 会把服务暴露到局域网 / 公网，请只在明确需要时这么做。

## 前置插件：`depends_on`

```toml
depends_on = ["base_lib", "net_core"]   # 插件 id，不是显示名
```

> TOML 语法上 `depends_on` 是顶层键，必须写在第一个表（`[[config]]` / `[startup]` / `[[tools]]`）**之前**，否则会被解析成那个表的字段。

声明本插件依赖哪些插件。与「前置有没有启动函数」无关——它首先是启用条件。

- **前置必须已安装且已启用**，否则本插件无法启用，设置页会弹窗说明缺谁。
- 启动时，本插件的启动函数会等前置的启动函数**执行完**再执行；前置没有启动函数则无需等待。互不依赖的插件之间仍然并行。
- **循环依赖会被检测出来**：环上的插件都不会启动，并被自动禁用、给出提示。
- **前置被禁用或卸载时，依赖它的插件会被自动禁用**（前置没了之后它调用上游工具会直接失败）。含传递依赖。
- **不自动恢复**：前置重新启用后，下游插件需要手动重新启用（卡片上会显示被禁用的原因）。
- 只有**启用与启动**时会查前置；导入插件时不校验，前置可以稍后再装。

## 启动入口：`[startup]`

```toml
[startup]
script = "boot.py"
handler = "on_start"
timeout_ms = 30000         # 可选，单次尝试的超时，默认 30000，上限 120000
retries = 3                # 可选，默认 0，上限 5
retry_interval_ms = 5000   # 可选，默认 5000，上限 60000
required = true            # 可选，默认 true
```

**程序启动时**、以及**插件在设置页被启用时**，各执行一次 `handler(ctx)`。

`retries = 3` 指「首次失败后**再试 3 次**」，即最多执行 4 次。

### handler 的 `ctx`

| 字段               | 类型     | 说明       |
| ------------------ | -------- | ---------- |
| `ctx["config"]`    | dict     | 同工具脚本 |
| `ctx["env"]`       | dict     | 同工具脚本 |
| `ctx["call_tool"]` | function | 同工具脚本 |

启动没有信号名与载荷，所以**没有** `signal` / `payload` / `tool_name` / `args`。

```python
# data/plugins/my_plugin/boot.py
def on_start(ctx):
    ctx["call_tool"]("memory_add_note", {"content": "插件已启动", "tags": ["system"]})
```

### 失败与重试

- `timeout_ms` 是**每次尝试各自**的超时。最坏情况单个插件会占用 `retries × (timeout_ms + retry_interval_ms)`；全按上限算约 15 分钟。**只有依赖它的插件会等**，无关插件照常并行推进、不会受影响。
- `required = true`（默认）：所有尝试都失败后，插件会被**自动禁用**，设置页的开关会自动关回去并显示原因。
- `required = false`：失败只记录原因，插件保持启用。适合「连不上也不该整个挂掉」的初始化，比如连接外部服务。
- 重试期间插件**保持启用**，开关不会动——只有彻底放弃时才关。

### 两个硬约束

**正在执行的脚本无法中断。** 插件被禁用时：等待中的重试会立刻停止，还没开始的调用不会再启动；但**已经进入脚本的执行只能等它自己返回**，返回值被丢弃。所以启动函数要快速返回，别写死循环。

**启动函数必须可重复执行。** 重试会再跑一次，而上次失败留下的副作用（写了一半的文件等）**不会回滚**。要么保证幂等，要么在开头自己清理。

## 内置工具 API 清单

以下 32 个工具可直接通过 `call_tool(name, args)` 调用（`execute_command` 仅桌面端注册）。成功返回一律是带 `"ok": true` 的 JSON 对象；失败返回 `{"ok": false, "error": {...}}`（见上）。

### 时间

**`get_current_time`**

- 参数：`{}`
- 返回：`{ local_time: string(RFC3339), timezone: string(固定 "local"，非真实时区名), unix_timestamp: number(秒) }`

### 日程（读写 `game_data/schedules.json`）

**`schedule_get_all`**

- 参数：`{}`
- 返回：`{ ok: true, ... }`，完整日程配置，顶层三键（camelCase，均可能为 null）：
  - `scheduleGroups`：定时日程分组
  - `todoGroups`：待办分组，`todoGroups[*]` = `{ title, description, todos: [{ id, text, priority, completed, deadline }] }`
  - `importantDays`：重要日子，`importantDays[*]` = `{ id, date, title, desc, cycle }`

**`schedule_add_todo`**

- 参数：`{ text: string(必), group?: string(默认 "default"), priority?: integer(默认 0), deadline?: string }`
- 返回：`{ ok: true, id: number, group: string }`

**`schedule_update_todo`**

- 参数：`{ id: integer(必), done?: boolean, text?: string, priority?: integer, group?: string }`（done/text/priority 至少一项；`group` 仅在旧数据同一 ID 跨分组重复时用于唯一定位）
- 返回：`{ ok: true, id: number, group: string }`

**`schedule_delete_todo`**

- 参数：`{ id: integer(必), group?: string }`（`group` 同上）
- 返回：`{ ok: true, id: number, group: string }`

### 记忆（角色笔记文件 + 自动记忆库）

**`memory_get_current`**

- 参数：`{}`
- 返回：`{ ok: true, role_id: number, memory: string }`（当前角色的自动记忆库文本）

**`memory_get_notes`**

- 参数：`{ role?: string }`（不传读当前角色；传其他角色名只读）
- 返回：`{ ok: true, notes: [ { id, content, tags: string[], created_at: string(RFC3339) } ] }`

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
- 返回（`{ ok: true, ... }`）：
  - `player`：玩家名
  - `current_role_id`：当前对话角色 ID（可能为 null）
  - `onstage_role_ids`：舞台角色 ID（保持出场顺序）
  - `present_role_ids`：在场角色 ID（升序；只有在场的角色能感知台词）
  - `main_role_id`：剧本模式主角 ID（可能为 null）
  - `background` / `present_pic` / `background_music` / `background_effect`：当前背景图 / 立绘 / 音乐 / 特效标识（无则为空串）
  - `current_scene_id`：当前场景 ID（可能为 null）
  - `scene_awareness_enabled`：场景感知开关（关后切场景不触发旁白）
  - `global_variables`：全局变量表（键 → 任意 JSON 值）

**`status_get_scene`**

- 参数：`{}`（未选择场景时报错）
- 返回：`{ ok: true, current_scene_id: string, name: string, description: string, background: string }`

### 场景（读写 SceneStore）

**`scene_list`**

- 参数：`{}`
- 返回：`{ ok: true, scenes: [ { id, name, description, background } ] }`

**`scene_switch`**

- 参数：`{ id?: string, name?: string }`（提供其一）
- 返回：`{ ok: true, scene_id: string }`

### 角色（读写数据库 + game_status）

> 角色有两个名字，界面上两处分别用它们，所以报告「现在是哪个角色」的工具两个都给：
> `name` 是**对话里显示的 AI 名称**（`settings.yml` 的 `ai_name`，如「风雪」），
> `title` 是**角色标题**（`settings.yml` 的 `title`，如「可爱的小风雪」，角色卡列表页那行大字）。

**`character_list`**

- 参数：`{}`
- 返回：`{ ok: true, characters: [ { id: number, name: string, title: string } ] }`

**`character_switch`**

- 参数：`{ id: integer(必) }`（不清空对话历史；`id` 不存在时报错并列出可用角色）
- 返回：`{ ok: true, role_id: number, name: string, title: string }`

**`character_get_clothes`**

- 参数：`{ role_id?: integer }`（省略时查当前对话角色）
- 返回：`{ ok: true, role_id: number, name: string, clothes_name: string, clothes: string[] }`（`clothes` 为该角色可更换的全部服装名）

**`character_set_clothes`**

- 参数：`{ name: string(必), role_id?: integer }`（省略 `role_id` 时换当前对话角色；`name` 须为 `character_get_clothes` 返回的服装之一，否则报错并列出可选值）
- 返回：`{ ok: true, role_id: number, name: string, clothes_name: string, switched: boolean }`（`switched: false` 表示本来就是这套，不会重复生成换装旁白）

> 换装会立即刷新立绘并往对话里写一句换装旁白（形如「XX换上了新服装：YY，…」），所以插件不需要再自己补一句描写。

### 搜索

**`web_search`**

- 参数：`{ query: string(必，过长截断) }`
- 返回：`{ ok: true, query: string, result_count: number, text: string }`（`text` 为排好版的搜索结果；未开启网页搜索时报错）

### 技能

**`list_skills`**

- 参数：`{}`
- 返回：`{ ok: true, skills: [ { name, location, description } ] }`

**`read_skill`**

- 参数：`{ name: string(必，kebab-case) }`
- 返回：`{ ok: true, name: string, base_directory: string, content: string }`（技能不存在时报错）

### 媒体

**`ReadMediaFile`**（唯一的大驼峰工具名）

- 参数：`{ path: string(必), prompt?: string, region?: { x, y, width, height }(4 个必填，仅图片), full_resolution?: boolean(仅图片) }`
- 返回：`{ ok: true, path, kind: "image"|"video", mime_type, source_bytes, delivered_bytes, dimensions, vision_model, analysis }`（`dimensions` 图片为 `{ original_width, original_height, delivered_width, delivered_height }`，视频为 null）

### 文件（受文件沙箱限制，默认 `data/`）

**`list_files`**

- 参数：`{ path: string(必) }`
- 返回：`{ ok: true, path: string, entries: [ { name, kind: "dir"|"file"|"symlink" } ], truncated: bool }`

**`read_file`**

- 参数：`{ path: string(必) }`
- 返回：`{ ok: true, path: string, content: string, truncated: bool }`（上限 200KB；图片/视频返回 `{ ok: false, error: { code: "media_file" } }` 提示改用 `ReadMediaFile`）

**`write_file`**

- 参数：`{ path: string(必), content: string(必), append?: boolean }`
- 返回：`{ ok: true, path: string, bytes: number, appended: bool }`

**`delete_file`**

- 参数：`{ path: string(必) }`（仅文件，不能删目录；默认弹窗确认）
- 返回：`{ ok: true, path: string }`

**`edit_file`**

- 参数：`{ path: string(必), old_string: string(必，须唯一匹配), new_string: string(必), replace_all?: boolean }`
- 返回：`{ ok: true, path: string, replacements: number }`

**`search_files`**

- 参数：`{ path: string(必), pattern: string(必，`\*`/`?` 文件名通配，大小写不敏感) }`
- 返回：`{ ok: true, pattern: string, matches: [ path ], truncated: bool }`

**`glob`**

- 参数：`{ pattern: string(必，支持 `\*\*`), path?: string, max_results?: integer(默认 100，上限 100) }`
- 返回：`{ ok: true, pattern: string, matches: [ path ], truncated: bool }`

**`grep_files`**

- 参数：`{ path: string(必), pattern: string(必，正则), max_results?: integer(默认 50，上限 100) }`
- 返回：`{ ok: true, pattern: string, output_mode: "content", matches: [ { path, line: number, text } ], truncated: bool }`

**`grep`**

- 参数：`{ pattern: string(必，正则), path?: string, glob?: string(文件过滤), case_insensitive?: boolean, output_mode?: "content"|"files_with_matches"|"count", max_results?: integer(默认 50) }`
- 返回：`{ ok: true, pattern, output_mode, matches: [...], truncated }`（`matches` 元素随 `output_mode` 变化：`content` → `{ path, line, text }`；`files_with_matches` → path 字符串；`count` → `{ path, count }`）

### 命令（仅桌面端）

**`execute_command`**

- 参数：`{ command: string(必), cwd?: string(默认沙箱根), uac?: boolean(仅 Windows), timeout_seconds?: integer(前台默认 60/最大 300，后台默认 600/最大 3600), run_in_background?: boolean, description?: string(后台必填) }`
- 返回（前台）：`{ ok: true, exit_code: number, output: string }`——**命令成败看 `exit_code`**（0 为成功），`ok` 只表示调用成功
- 返回（后台）：`{ ok: true, task_id, description, status: "running", message }`（完成后自动通知模型，无需轮询）

## 插件系统的私有 API（非 llm 可调用工具）

这些能力**只有插件脚本能用**，不注册进 `ToolRegistry`，因此 LLM 选不到它们。放进这一类通常是因为「不该让模型自己决定」——比如会丢弃对话历史的破坏性操作。

和 `http_get` / `http_post` 同一个模块，用 `from plugin_host import ...` 取用。返回统一信封：成功 `{ "ok": true, ... }`，失败 `{ "ok": false, "error": "..." }`，**不抛异常**。工具脚本、信号 handler、启动入口里都能调。

### `switch_character(role_id)`

完整切换当前角色，与玩家在角色卡上点「切换角色」走的是**同一条路径**：

- **会清空当前对话历史**，重置已加载角色与角色内存，清空在场角色；
- 递增会话边界代号，把旧一轮生成中迟到的台词丢掉，避免 A 的台词串进 B 的对话；
- 把新的整份游戏状态推给前端，前端整体替换并丢弃旧事件队列。

返回：成功 `{ "ok": true, "role_id": 3, "name": "风雪", "title": "可爱的小风雪" }`；角色不存在 `{ "ok": false, "error": "角色 id 3 不存在" }`。

`name` / `title` 的语义与 `character_list`、`character_switch` 一致（见上面「角色」一节的说明）。

```python
# data/plugins/story_switch/boot.py
from plugin_host import switch_character

def on_start(ctx):
    r = switch_character(3)
    if not r["ok"]:
        print("切换失败:", r["error"])
```

> **想保留对话历史地换角色**，请改用 `ctx["call_tool"]("character_switch", {"id": 3})`——那是 LLM 工具，只换角色、不动历史。
>
> `role_id` 会先校验存在性再动手，所以写错 id 只会拿到 `ok: false`，不会先把你的对话清空。

### 台词历史（上下文源）：`read_context` / `edit_context` / `compress_context`

这三个函数操作**当前对话的台词历史**（内部叫 `line_list`）——即「LLM 上下文」的**源**：每轮发给 LLM 的上下文都是它按各角色视角渲染出来的。三点务必记牢：

- 下标按**台词行**算（不是渲染后的 LLM 消息），**从 1 开始、闭区间**，第 1 条通常就是 role system 的人设行。
- 改的是**全局历史**：影响所有角色看到的上下文 + 界面显示的历史 + 存档。
- 正在生成回复时 `edit_context` 会被**拒绝**（避免和流式写入打架）。

**`read_context(start=None, end=None)`** — 读第 `start`~`end` 条，**省略 = 整段**。

返回 `{ "ok": true, "total": N, "lines": [ ... ] }`。每行字段齐全：`id`（未存盘的可能为 null）、`content`、`original_emotion`、`predicted_emotion`、`tts_content`、`action_content`、`audio_file`、`thinking`、`tool_call`、`attribute`、`sender_role_id`、`display_name`、`perceived_role_ids`。

> `attribute` 取值是 `"System"` / `"User"` / `"Assistant"` / `"Tool"`（首字母大写）。

```python
from plugin_host import read_context

r = read_context()                 # 整段
r = read_context(start=1, end=3)   # 第 1~3 条（含人设行）
for line in r["lines"]:
    print(line["attribute"], line["display_name"], line["content"])
```

**`edit_context(replacement, start=None, end=None)`** — 用 `replacement`（**台词行 dict 列表**）替换第 `start`~`end` 条，**省略区间 = 整段**。

- 条数不限：比被替换区间**少**即合并（如 3 行写 1 行）、**多**即展开；传**空列表**即删除该区间。
- 每个 dict 必须**字段齐全**（最省事是拿 `read_context` 的输出改）。**不带 `id` 的行会按位置继承被替换行的 id**，保住存档链锚点，一般不用自己填。
- System 人设行**可以编辑 / 删除**；删掉后该角色上下文就没有 system 前缀了（宿主只记一条警告，不拦截）。
- **落库要等存盘**：本函数只改内存并立刻刷新上下文，写进存档是之后正常存盘（手动 / 自动存档）的事。

返回 `{ "ok": true, "removed": N, "total": M }`（`removed` = 被替换掉的条数）；生成中返回 `{ "ok": false, "error": "正在生成回复，暂不能编辑历史，请稍后再试" }`。

```python
from plugin_host import read_context, edit_context

# 把第 1~3 条合并成 1 条旁白
lines = read_context(start=1, end=3)["lines"]
merged = lines[0]
merged["content"] = "（把前面三句合成了一句）"
merged["attribute"] = "User"
r = edit_context([merged], start=1, end=3)   # {"ok": True, "removed": 3, "total": ...}
```

**`compress_context()`** — 立刻触发一次永久记忆压缩并**等它完成**再返回（只压达到阈值的角色；没开永久记忆 / 没配 LLM / 没到阈值都直接返回）。相当于「把这段剧情沉进长期记忆」的显式触发点，会调用若干次 LLM，**可能耗时数秒**。

返回 `{ "ok": true, "triggered": N }`（`N` = 实际触发压缩的角色数，0 = 无需压缩）。

```python
from plugin_host import compress_context
r = compress_context()   # {"ok": True, "triggered": 1}
```

> **和永久记忆的关系**：如果永久记忆已经把早期台词压缩成了摘要，那些行**仍在历史里，但发给 LLM 的是摘要**——只改行对 LLM 无效（system 人设行除外）。`edit_context` 会自动把压缩指针回拨到编辑处，让这段重新进入上下文；随后的压缩把改动重新摘要进去。代价是**逐字内容会被 LLM 改写成摘要**，所以想保留原文就别在改动后紧接着压缩。

### WebSocket：`ws_open` / `ws_close` / `ws_send` / `ws_status`

操作本插件 `[[ws]]` 声明的连接（见「WebSocket 连接」一节）。只会操作**自己声明过的**连接，别的插件的连接碰不到。

**`ws_open(conn_id, url=None)`** — 启用 / 重连一个连接。插件启用时宿主已自动建立全部连接，本函数用于关闭后重连，或用 `url` 覆盖声明值（仍受 `ws_allow` 白名单约束）。返回 `{ "ok": true }` 或 `{ "ok": false, "error": "..." }`（如未声明该连接、url 不在白名单）。

**`ws_close(conn_id)`** — 关闭一个连接。返回 `{ "ok": true }`。

**`ws_send(conn_id, data)`** — 向连接发一帧。`str` / `bytes` / 其余（dict、list、数字，转 JSON）分别对应文本帧 / 二进制帧 / JSON 文本帧。**同步发出、不等回执**；要拿对端响应等下一次 `ws_message` 事件。返回 `{ "ok": true }` 或 `{ "ok": false, "error": "..." }`（如连接未建立）。

**`ws_status(conn_id=None)`** — 查询连接状态。不传返回全部声明：`{ "ok": true, "connections": [ { "id", "mode", "state" } ] }`；传 `conn_id` 返回单条：`{ "ok": true, "id", "mode", "state" }`。`state` 取 `"connected"` / `"connecting"` / `"stopped"` / `"error"`。

```python
from plugin_host import ws_send, ws_close, ws_status

def on_start(ctx):
    ws_send("gateway", {"op": 1})        # 发一帧
    r = ws_status("gateway")             # {"ok": True, "id": "gateway", "mode": "client", "state": "connected"}
    # ws_close("gateway")                # 临时断开
```

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
- 脚本无法直接写文件系统、启动子进程、加载系统库；读也只有一个口子——`read_data_file`，且只能读 manifest `read` 声明过的 `data/` 子目录（未声明 = 一律拒绝，见「读游戏素材」一节）。
- 网络出站：`http_get` / `http_post` 可发往任意 URL；WebSocket client 连接受 manifest `ws_allow` 白名单约束（省略 `ws_allow` = 允许全网段），且脚本只能操作自己在 `[[ws]]` 里声明过的连接。
- 每次调用新建解释器，无跨调用状态；超时（`timeout_ms`，上限 120000ms）后执行结果作废、本次调用终止。
- **注意**：超时无法强制中断脚本所在的阻塞线程，死循环可能残留占用线程直至进程退出，插件作者（和你们的agent）应避免写死循环。
- `call_tool` 是有意的受信任通道，可触达所有注册工具（含写操作）。（谨慎使用）
