## 背景

随着项目工具注册的规模越来越大，整体下来可能给 AI 注册的工具包括超过 30 个工具，尤其是还有插件外部扩展的情况下。为了保证 AI 的工具注册以及调用更好一点，现在提议建议使用 Tool Rag 机制来进行工具注册以及调用。

## 大致的思路

1. 实现 Tool Rag 的接口，把原来所有注册的工具灌输给上下文改成在 System Prompt 里面改成提示：“当你想为我做些什么，或者我要求你做什么的时候，你可以通过 tool_search 工具函数来查询是否有满足需求的工具，并尽可能调用已有工具，假如没有合适的工具可以承认自己做不到。
2. 确定 RAG 实现方案，尽量选择轻量化的方案。由于记忆检索系统以后可能也要用，选择合适的 RAG 方案。

## 注意事项

1. 竟可能不要对现有代码有较大的破坏
2. 可以设置个工具搜索缓存，避免每次都去查询工具。
3. 每次 RAG 搜索候选 5 个工具，并进入缓存。缓存中的工具采用类似 LRU 的机制，当缓存满了以后，把最久没有使用的工具移除。

---

## 实现

已按上述提案落地。与提案有出入的地方在下面逐条说明。

### 一个前置澄清

工具本来**不是**写在 System Prompt 里枚举的，而是通过原生 function-calling 的 `tools` 数组下发
（`tools/tool_loop.rs` 的 `stream_with_tool_loop` → `ToolRegistry::definitions_for_allowed`
→ `LlmClient::complete_stream_with_tools`）。所以本功能的实质是：**把下发的工具定义从
「权限全集」收窄为「常驻工具 + 检索激活的工具」**，再补一段 system 提示，让模型知道
「先去检索」这件事本身存在。

### 检索方案

ONNX 语义向量检索，模型 `bge-small-zh-v1.5`（512 维，uint8 量化约 23MB，单文件）。
代码在 `crates/ling-chat-main/src/ai_service/embedding/`，API 只有
`EmbeddingEngine::embed` 与 `cosine_similarity`，**将来记忆检索可直接复用挂在
`AppState` 上的同一个实例**（`state.embedding`）。

选它而不是词法检索，是因为语义检索对「换个环境」→ `scene_switch` 这类非同词命中更稳；
选它而不是更大的多语言模型，是因为工具只有 30~40 个、描述全中文，且模型文件会进每个
安装包（`prepare-desktop-resources.mjs` 整目录递归复制 `data/third_party/`）。

模型与分词文件不进 git（`.gitignore` 的 `data/**` 排除整个 `data` 目录），靠
`scripts/download_embedding_model.mjs` 获取；`pnpm run init` 已包含这一步。
换模型只需要改 `embedding/engine.rs` 的 `MODEL_DIR_NAME` 与下载脚本的 `BASE_URL` / `FILES`。

### 常驻工具

**`tool_search` 与 `web_search`** 不走检索、每轮直接下发。`web_search` 仍受它自己的
开关与权限矩阵约束，不会被常驻强行放行。

### tool_search 的动态描述（关键）

只给模型一个「你可以检索」的入口、不给任何能力线索，模型不会想到去检索 —— 它会直接
回答做不到。所以 `tool_search` 的描述在**每轮下发前按当前可用工具重建**：把权限集里
除常驻工具以外的工具列成一份清单（工具名 + 一句用途，用途取描述第一句、过长时切在
逗号处），再附上「用一句话描述你想做的事调用本工具取证具体参数」的引导。

清单只到名字与用途，**参数不在其中** —— 具体用法依旧靠检索结果取得，token 收益不变。
真实注册表（35 个工具）下清单约 1.5K 字符量级，而完整 schema 要上万字符；跑应用时开
debug 日志可以看到实测的 `sent_chars` / `full_chars`（见 `tool_loop.rs` 的
「Tool RAG 工具集已收窄」）。

渲染逻辑在 `tool_rag.rs` 的 `search_description` / `apply_dynamic_search_description`，
由 `tool_loop.rs` 在算出本轮 `allowed` 后调用（`base_allowed` 决定清单内容，因此清单
天然不会泄漏该角色无权使用的工具）。

### 缓存

按**角色**为键，进程内跨回复保留，容量 10，超出时淘汰最久未被检索激活的。
实现就是 `Vec<String>`（队首最重要），元素只有十来个，不值得引入 `lru` crate。
工具向量的缓存按「工具名 + 描述指纹」失效，因此插件热重载/卸载不需要任何主动通知。

### 权限不变量

**权限永远优先。** `tool_search` 只在「该角色/场景本来就至少有一个可用工具」
（权限集非空）时才注入；检索语料只从权限集里取；激活缓存读取时始终与权限集求交。
因此收窄工具集不可能带来任何权限提升。该不变量有单测覆盖
（`tool_rag.rs` 的 `effective_set_stays_within_permissions`）。

收窄只决定**下发**，不决定**放行**：`ToolExecutor` 按「权限集 ∪ 本轮下发集」复核，
所以模型凭记忆直接调一个本轮没下发的工具照样执行——缓存只在进程内，重启就空，
而模型的记忆是持久的，拿「本轮没下发」当拒绝理由等于把缓存未命中变成对玩家的
「工具被系统拦下来了」（见 [permission.md](permission.md) §6）。这种情况下
`tool_loop` 会 `touch` 它，从下一轮起完整参数表重新下发。

反过来，放行也**不能只**看权限集：`tool_search` 按上面的不变量就在权限集之外，
只认权限集等于把检索入口自己锁死。

### 降级

模型缺失、`onnxruntime.dll` 不可用、分词或推理失败、当前 provider 不支持原生流式工具时，
一律回退到「全量工具下发」（即本功能之前的行为）并记日志。设置页有一个默认开启的开关
（`tool_settings.tool_rag`）可以手动关闭。

### 已知代价

工具首次使用会比原来多一次 LLM 往返（先检索、再调用），token 成本与延迟都上升。
按角色的激活缓存让同一角色已检索过的工具在后续回复里持续可见，是主要的缓解手段。

缓存**不跨进程**：重启之后模型凭记忆调工具（台词表里还留着上次的检索结果）会被
重新激活并照常执行，但那一轮它看不到参数表，只能靠记忆里的旧 schema 猜。
真要避免，得在重启后的第一轮先检索一次——目前不做这件事。

### 验证

```bash
# 拉模型
node scripts/download_embedding_model.mjs

# 纯逻辑单测（不需要模型）：LRU、权限求交、语料排除、余弦相似度
cargo test -p ling-chat-main --lib

# 真实模型下的嵌入冒烟：维度、归一化与语义区分度。
# 需先拉模型；单独运行（同一进程里 ort::init_from 只应初始化一次）
cargo test -p ling-chat-main --lib embedding_smoke -- --ignored --nocapture
```
