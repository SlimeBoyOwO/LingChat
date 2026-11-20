# LingChat × OneBot v11（QQ 桥）

把 [LingChat](https://github.com/SlimeBoyOwO/LingChat) 接到 QQ 群 / 私聊的插件，协议走标准 **OneBot v11**，
推荐搭配 [SnowLuma](https://github.com/SnowLuma/SnowLuma)（QQ NT 的 OneBot 实现）。

- 群里 @ 她或带前缀才会理你，私聊直接聊
- 她**看得见群里别人在说什么**（上下文缓冲，可让她主动查）
- 立绘跟着情绪走：每条回复自动配当前角色的表情图
- 回复整轮合并 / 一句一条，两种节奏都能调
- 白名单、限速、管理员、引用回复、@ 回复都有
- 不依赖任何外部服务（不需要自建后端），配个群号就能跑

## 它长什么样

```
群消息 → 唤醒判定 → 送进 LingChat 对话 → 角色逐句上屏 → 回到群里
                        ↑                              ↓
                  群聊上下文缓冲                  情绪 → 立绘配图
```

## 安装

1. 把 `qq_onebot/` 整个目录放到 LingChat 的 `data/plugins/qq_onebot/`
   （目录名必须和 `manifest.toml` 里的 `id` 一致）
2. 重启 LingChat，在 **设置 → 插件** 里启用「QQ 桥接（OneBot v11）」
3. 填配置（见下），保存后重启生效

## 配置 OneBot 端

以 SnowLuma 为例：

1. 启动 SnowLuma，登录 QQ，把 QQ 进程接入（inject）
2. 打开它的控制台，建一个 **WebSocket 服务端**（ws server），比如 `127.0.0.1:3001`，记下 access token
3. 插件里填：

```
ws_url        = ws://127.0.0.1:3001
access_token  = 刚才那个 token
```

其他实现（NapCat / LLOneBot / go-cqhttp）同理，只要是 OneBot v11 的 ws 服务端就行。

> 插件用 `Authorization: Bearer <access_token>` 头认证。如果你的实现只认 `?access_token=`，
> 把 token 直接拼在 `ws_url` 后面也可以。

## 插件配置

### 唤醒规则

| 键 | 默认 | 说明 |
| --- | --- | --- |
| `wake_prefix` | `/` | 前缀唤醒，多个用逗号分隔（如 `/ ,鱼`） |
| `at_required_in_group` | `true` | 群聊必须 @ 她或带前缀 |
| `friend_needs_prefix` | `false` | 私聊也必须带前缀 |
| `ignore_bot_self` | `true` | 忽略自己发的消息 |
| `ignore_at_all` | `true` | 忽略「@全体成员」 |
| `enable_whitelist` | `false` | 只有白名单里的会话才理 |
| `whitelist` | | `group:123456,private:654321` 形式 |
| `max_len` | `300` | 超长消息截断 |
| `admins` | | 管理员 QQ，可绕过限速 |

### 回复行为

| 键 | 默认 | 说明 |
| --- | --- | --- |
| `reply_with_quote` | `false` | 引用回复 |
| `reply_with_mention` | `false` | @ 对方回复 |
| `merge_lines` | `true` | 整轮合并成一条 |
| `max_lines_per_turn` | `3` | 攒够几句发一条（`1` = 一句一条，真流式） |
| `line_gap_seconds` | `2` | 相邻两句间隔超过这个值就认为上一批说完了 |
| `max_chars_per_message` | `300` | 单条上限，超了按句号切开 |
| `reply_length_mode` | `short` | 连上时调 LingChat 的回复长度档（`short`/`normal`/`free`） |

### 群聊上下文

| 键 | 默认 | 说明 |
| --- | --- | --- |
| `group_context_enabled` | `true` | 记录群消息（不管叫没叫到她） |
| `group_context_size` | `12` | 缓冲多少条 |
| `group_context_inject` | `6` | 叫醒她时附带几条 |
| `group_context_ttl_s` | `600` | 缓冲有效期 |

### 立绘表情

| 键 | 默认 | 说明 |
| --- | --- | --- |
| `emotion_image_enabled` | `true` | 回复时带上表情图 |
| `emotion_image_dir` | | 表情图目录（每个情绪一张，如 `正常.webp` / `调皮.webp`） |
| `emotion_image_position` | `after` | `after` 文字在上 / `before` 图在上 |

表情图按 LingChat 角色目录里的 `avatar/<情绪>.webp` 来。想裁成上半身特写：

```bash
python tools/crop_upper.py "C:/.../characters/<角色>/avatar"
```

多角色还想各自用各自的表情，再生成一次映射表：

```bash
python tools/gen_characters.py
```

### 单目标 vs 多群

默认 **单目标模式**：配 `default_target_group`（或 `default_target_user`），所有回复都发到那儿，
不需要任何外部服务，代价是一句一条、没有限速。

想要「多群并发 + 整轮合并 + 限速」，需要一个能跨调用存状态的 HTTP 后端，配 `state_url` / `state_token`
（插件在两次回调之间没有共享内存，LingChat 每次调用都新建解释器）。

## AstrBot 配置对照

从 AstrBot 迁过来基本一对一：

| AstrBot | 本插件 |
| --- | --- |
| `wake_prefix` | `wake_prefix` |
| `at_required` | `at_required_in_group` |
| `friend_needs_prefix` | `friend_needs_prefix` |
| `ignore_bot_self` | `ignore_bot_self` |
| `ignore_at_all` | `ignore_at_all` |
| `whitelist` | `enable_whitelist` + `whitelist` |
| `rate_limit` | `rate_time` + `rate_count` |
| `unique_session` | `unique_session` |
| `reply_with_quote` | `reply_with_quote` |
| 群聊上下文 | `group_context_*` |
| 分段回复 | `merge_lines` + `max_lines_per_turn` |

## 提供的工具

| 工具 | 作用 |
| --- | --- |
| `qq_status` | 看连接状态、当前回复目标、命中过的规则 |
| `qq_context` | 主动看某个群最近在聊什么（不用等被 @） |
| `qq_send` | 主动往指定群/私聊发一条（可带情绪立绘） |

## 已知限制

- 需要 **LingChat 0.6.0+** 的插件 API（`send_user_message` / `[[ws]]` / `[[subscribe]]`）。
  0.5.x 的插件系统是 RustPython，没有这些能力。
- 插件脚本运行在 LingChat 的受限解释器里：`os` / `math` / `subprocess` 之类不可用，
  所以表情映射表是静态生成的（见 `tools/gen_emotions.py`）。
- 回调之间没有共享内存，多群场景必须配状态后端（见上）。
- 一次回复如果被拆成多条，顺序由发送队列保证；QQ 服务器偶尔仍可能乱序显示。

## 目录

```
qq_onebot/
├── manifest.toml   # 插件声明：配置项 + 3 个工具 + ws 连接 + line_shown 订阅
├── main.py         # 工具实现（qq_status / qq_context / qq_send）
├── hooks.py        # 回调：on_ws（收消息）/ on_line_shown（发回复）
└── rules.py        # 纯函数：唤醒判定、会话 key、情绪→立绘映射
tools/
├── crop_upper.py     # 立绘裁成上半身特写
├── gen_emotions.py   # 生成「情绪 → 文件名」静态表
└── gen_characters.py # 生成「角色名 → 立绘目录」映射（含显示名别名）
```

## License

MIT
