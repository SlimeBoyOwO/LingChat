# LingChat 项目前后端通讯 API 开发文档

本文档描述了 LingChat 项目中用于数据库交互、角色管理、系统初始化及资源获取的 HTTP API 接口。旨在协助前端与后端进行分立开发。

## 1. 基础说明

*   **基础 URL**: `http://<host>:<port>/api/v1` (默认端口通常为 8765)
*   **数据格式**: 请求体 (Request Body) 和 响应体 (Response Body) 通常为 JSON 格式。
*   **静态资源**: 部分接口返回文件流 (FileResponse)，用于图片、音频等资源加载。

---

## 2. 聊天历史与会话管理 (Chat History)

**路由前缀**: `/chat/history`

该模块主要负责管理用户的对话记录数据库（SQLite），包括列表获取、加载存档、新建存档、保存及删除。

### 2.1 获取用户对话列表

分页获取指定用户的历史对话记录。

*   **接口地址**: `/list`
*   **请求方式**: `GET`
*   **请求参数 (Query Params)**:

| 参数名 | 类型 | 必选 | 描述 | 示例 |
| :--- | :--- | :--- | :--- | :--- |
| `user_id` | int | 是 | 用户ID | `1` |
| `page` | int | 否 | 页码，默认 1 | `1` |
| `page_size` | int | 否 | 每页数量，默认 10 | `10` |

*   **响应示例**:

```json
{
    "code": 200,
    "data": {
        "conversations": [
            {
                "id": 1,
                "title": "与钦灵的第一次对话",
                "updated_at": "2023-10-27 10:00:00",
                "last_message_id": 50,
                "created_at": "2023-10-27 09:00:00"
            }
        ],
        "total": 1
    }
}
```

### 2.2 加载对话 (恢复记忆)

将指定的历史对话记录加载到后端 AI 服务的内存中，以便继续对话。同时会切换 AI 的当前角色配置。

*   **接口地址**: `/load`
*   **请求方式**: `GET`
*   **请求参数 (Query Params)**:

| 参数名 | 类型 | 必选 | 描述 |
| :--- | :--- | :--- | :--- |
| `user_id` | int | 是 | 用户ID |
| `conversation_id` | int | 是 | 对话ID |

*   **响应示例**:

```json
{
    "code": 200,
    "data": "success"
}
```

### 2.3 创建新对话

将当前 AI 内存中的消息记录保存为数据库中的一条新会话记录。

*   **接口地址**: `/create`
*   **请求方式**: `POST`
*   **请求头**: `Content-Type: application/json`
*   **请求体**:

```json
{
    "user_id": 1,
    "title": "新的对话存档"
}
```

*   **响应示例**:

```json
{
    "code": 200,
    "data": {
        "conversation_id": 102,
        "message": "对话创建成功"
    }
}
```

### 2.4 保存/更新对话

将当前 AI 内存中的消息更新到已存在的数据库会话中。

*   **接口地址**: `/save`
*   **请求方式**: `POST`
*   **请求头**: `Content-Type: application/json`
*   **请求体**:

```json
{
    "user_id": 1,
    "conversation_id": 102,
    "title": "可选的新标题" 
}
```

*   **响应示例**:

```json
{
    "code": 200,
    "data": {
        "conversation_id": 102,
        "message": "对话保存成功",
        "message_count": 15
    }
}
```

### 2.5 删除对话

删除指定的会话记录。

*   **接口地址**: `/delete`
*   **请求方式**: `POST`
*   **请求头**: `Content-Type: application/json`
*   **请求体**:

```json
{
    "user_id": 1,
    "conversation_id": 102
}
```

*   **响应示例**:

```json
{
    "code": 200,
    "data": {
        "conversation_id": 102,
        "message": "对话删除成功"
    }
}
```

### 2.6 导入日志文件

解析前端上传的日志文本内容，将其存入数据库并加载到内存。

*   **接口地址**: `/process-log`
*   **请求方式**: `POST`
*   **请求体**:

```json
{
    "user_id": 1,
    "content": "日志文件的纯文本内容..."
}
```

*   **响应示例**:

```json
{
    "success": true,
    "processed_count": 20,
    "conversation_id": 105
}
```

---

## 3. 角色管理 (Chat Character)

**路由前缀**: `/chat/character`

该模块负责读取游戏数据目录中的角色信息、切换当前角色以及获取角色资源。

### 3.1 获取所有角色列表

读取 `data/game_data/characters` 下的角色配置并返回。

*   **接口地址**: `/get_all_characters`
*   **请求方式**: `GET`
*   **响应示例**:

```json
{
    "data": [
        {
            "character_id": 1,
            "title": "诺一钦灵",
            "info": "这是一个人工智能对话助手...",
            "avatar_path": "诺一钦灵/avatar/头像.png" 
        },
        {
            "character_id": 2,
            "title": "风雪",
            "info": "...",
            "avatar_path": "风雪/avatar/头像.png"
        }
    ]
}
```

### 3.2 切换角色

切换后端 AI 服务当前激活的角色，并重置内存。

*   **接口地址**: `/select_character`
*   **请求方式**: `POST`
*   **请求体**:

```json
{
    "user_id": 1,
    "character_id": 1
}
```

*   **响应示例**:

```json
{
    "success": true,
    "character": {
        "id": 1,
        "title": "诺一钦灵"
    }
}
```

### 3.3 获取角色文件 (静态资源)

获取角色的头像或其他资源文件。

*   **接口地址**: `/get_avatar/{avatar_file}`
*   **请求方式**: `GET`
*   **参数说明**: `avatar_file` 为文件名（如 `happy.png`）。
*   **注意**: 该接口依赖后端当前加载的 `ai_service` 角色路径。建议优先使用 `3.4` 接口获取特定文件。
*   **响应**: 文件流 (image/png 等)。

### 3.4 通过路径获取角色资源

*   **接口地址**: `/character_file/{file_path}`
*   **请求方式**: `GET`
*   **参数说明**: `file_path` 是相对于 `data/game_data/characters/` 的路径。
*   **响应**: 文件流。

### 3.5 刷新角色列表

重新扫描 `game_data` 目录并同步到数据库。

*   **接口地址**: `/refresh_characters`
*   **请求方式**: `POST`
*   **响应**: `{"success": true}`

---

## 4. 系统初始化信息 (Chat Info)

**路由前缀**: `/chat/info`

用于前端页面初始化时获取 AI 和用户的基本配置信息。

### 4.1 初始化 Web 信息

如果 AI Service 未初始化，该接口会基于用户的 `last_chat_character` 进行初始化。

*   **接口地址**: `/init`
*   **请求方式**: `GET`
*   **请求参数**: `user_id` (int)
*   **响应示例**:

```json
{
    "code": 200,
    "data": {
        "ai_name": "钦灵",
        "ai_subtitle": "AI Assistant",
        "user_name": "Admin",
        "user_subtitle": "User",
        "character_id": 1,
        "thinking_message": "灵灵正在思考中...",
        "scale": 1.0, 
        "offset": 0,
        "bubble_top": 5,
        "bubble_left": 20
    }
}
```

---

## 5. 资源管理 (背景与音乐)

此类接口主要用于前端获取静态资源或进行简单的文件管理。

### 5.1 背景图片 (Chat Background)
**路由前缀**: `/chat/background`

*   **获取背景列表**: `GET /list`
    *   返回: `{"data": [{"title": "name", "url": "filename.png", "time": "timestamp"}]}`
*   **获取背景图片**: `GET /background_file/{background_file}`
    *   返回: 图片文件流。
*   **上传背景**: `POST /upload` (Multipart Form Data)

### 5.2 背景音乐 (Chat Music)
**路由前缀**: `/chat/back-music`

*   **获取音乐列表**: `GET /list`
    *   返回: `[{"name": "stem", "url": "filename.mp3"}]`
*   **获取音乐文件**: `GET /music_file/{music_file}`
    *   返回: 音频文件流。
*   **上传音乐**: `POST /upload`
*   **删除音乐**: `DELETE /delete?url=filename.mp3`

### 5.3 剧本资源 (Chat Script)
**路由前缀**: `/chat/script`

*   **初始化剧本信息**: `GET /init_script/{script_name}`
    *   返回剧本配置、角色信息及 UI 布局参数。

---

## 6. 环境配置 (Env Config)

**路由前缀**: `/api/settings/config` (注意没有 v1)

*   **获取配置**: `GET /api/settings/config`
    *   返回 `.env` 文件的解析结构（包含分类、描述、当前值）。
*   **保存配置**: `POST /api/settings/config`
    *   请求体: `{"KEY": "VALUE", ...}`
    *   功能: 更新 `.env` 文件。

---

## 7. 实时通讯 (WebSocket)

虽然不是 HTTP API，但为了开发完整性，需注意主要的对话流通过 WebSocket 进行。

*   **Endpoint**: `/ws`
*   **协议**:
    *   **发送 (Client -> Server)**:
        *   普通对话: `{"type": "message", "content": "你好"}`
        *   开始剧本: `{"type": "message", "content": "/开始剧本"}`
    *   **接收 (Server -> Client)**:
        *   消息格式通常包含 `type` (reply/player/background等), `message`, `emotion`, `audioFile` 等字段（参考 `core/schemas/responses.py`）。