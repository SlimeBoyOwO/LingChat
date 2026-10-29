# 笔记向量化接入云端供应商 · 实施计划

状态：已实施（2026-10-27）
范围：笔记语义检索（`memory_get_notes`）与 Tool RAG 的嵌入后端
决策已定：默认本地 ONNX 小模型，云端为可选项；云端连续失败后跳过本轮向量化并告警

已确认的决策（本轮问答结论）：

- 覆盖范围：笔记检索与 Tool RAG **两处都走云端**。工具侧日后会做向量缓存落库，届时预热成本可摊薄。
- 凭据：云端嵌入**独立配置一套** provider / base_url / api_key / model，不复用大模型管理的 provider。
- 重建时机：保存配置后**自动后台预热**，不依赖用户下次检索触发。

## 1. 目标与不变量

当前嵌入能力只有一种实现：本地 ONNX（bge-small-zh-v1.5，512 维）。目标是让笔记的语义检索
可以改用云端 embedding 接口，同时保证三条不变量：

1. 未配置云端、或配置不完整时，行为与现在完全一致（本地小模型，默认态）。
2. 云端不可用时，功能退化但绝不阻塞对话：本轮跳过向量化、按原有降级路径返回，并让用户看得见原因。
3. 本地向量空间与云端向量空间永不混用，切换后端不会产生错误的相似度排序。

不做的事：rerank / 精排。genai 0.7.0-rc.8 没有 rerank 端点（`ServiceType` 只有
Chat/ChatStream/Embed，全仓零命中），自建 HTTP 的成本与收益要到本项落地、有基线数据后再评估。

## 2. 探明的现状（决定了方案的形状）

- 所有嵌入调用都已包在 `tokio::task::spawn_blocking` 闭包内
  （`ai_service/tools/builtin/memory.rs` 检索分支、`ai_service/tools/tool_rag.rs` 批量嵌入分支）。
  阻塞线程不是 async 上下文，因此**嵌入接口可以保持同步**，云端实现内部用 tokio
  `Handle::block_on` 驱动一次 genai 异步请求即可。整条检索链路的签名、并发模型、
  SQLite 访问模式都不用动。这是本方案最重要的前提。
- 索引已按模型分库：`<data_dir>/vector_index/notes/<模型目录>/<角色>.db`。换后端等于换一个
  目录，天然从空索引开始，不会把两种向量空间写进同一张表。第 3 条不变量已经免费拿到。
- 硬阻塞只有一个：`vector.rs` 的 `EMBEDDING_DIM = 512` 是编译期常量，同时写进 vec0 建表
  DDL 与写入校验。云端模型维度普遍是 1024/1536/2048，必须把它降为运行期参数。
- `ling_chat_notes` 在依赖链最底层（plugins → main → notes），不依赖 tauri 也不依赖 HTTP 栈。
  云端实现不能放进这个 crate，否则会拖入 hyper/tokio。抽象用 trait 留在 notes，
  云端实现放 main 的 `ai_service`。
- genai 侧结论（已核实）：`genai::embed` 模块完整可用，
  `client.embed / embed_batch / exec_embed` 均为 async，返回 `EmbedResponse`
  （`first_vector() / into_vectors()`，带 `dimensions` 与 usage）。
  OpenAI 协议走标准 `/embeddings`；`AdapterKind::Custom`、`DeepSeek`、`Baidu` 都委托
  OpenAI 实现，`Gemini`、`Cohere`、`Ollama` 有原生实现。
  显式不支持的：Anthropic、Groq、Kimi、MiniMax、Heabsy（返回 `AdapterNotSupported`）。
  DeepSeek 服务端实际不提供 embeddings 端点，配置项里不列为可选供应商。
- 配置落点现成：嵌入配置并入 `ToolSettings`（`ai_service/tools/settings.rs`），
  保存走 `api/tool_settings.rs::save_tool_settings` —— 那条路径本来就做写盘 + 热更新 +
  权限矩阵同步，云端配置的热重载直接搭这趟车，不需要新开 IPC 域。

## 3. 架构

```
ling_chat_notes                      ling_chat_main (ai_service)
┌────────────────────────┐           ┌──────────────────────────────┐
│ trait Embedder         │◄──impl────│ RemoteEmbedder (genai HTTP)  │
│  is_enabled/dimension  │           │ EmbeddingSelector            │
│  namespace             │           │  （本地/云端路由 + 失败态）   │
│  embed / embed_batch   │           └──────────────────────────────┘
└────────────────────────┘
        ▲impl
┌────────────────────────┐
│ EmbeddingEngine (ONNX) │  ← 保持现状，禁用即降级的宽容语义不变
└────────────────────────┘
```

AppState 与 ToolRag 持有的具体类型 `Arc<EmbeddingEngine>` 改为 `Arc<dyn Embedder>`；
`retrieval::search` 的 `&EmbeddingEngine` 参数改为 `&dyn Embedder`。调用点全部是类型替换，
无逻辑改动。

`Embedder` 接口（同步、阻塞、`Send + Sync`）：

| 方法                 | 说明                                                  |
| -------------------- | ----------------------------------------------------- |
| `is_enabled()`       | 是否可用（本地：模型齐备；云端：配置就绪且未熔断）    |
| `dimension()`        | 向量维度，用于建库。本地固定 512；云端从首次响应推断  |
| `namespace()`        | 索引分库目录名，本地用模型目录名，云端用供应商+模型名 |
| `embed(text)`        | 单条，L2 归一化                                       |
| `embed_batch(texts)` | 批量，L2 归一化                                       |

路由与失败态都由 `EmbeddingSelector`（实现同一个 `Embedder`）承担，因此下游只需要面对一个
嵌入器对象，看不出下面是本地还是云端。

## 4. 失败与熔断语义（按已定决策实现）

云端一次调用失败 → 内部重试，累计 5 次仍失败 → 进入**熔断态**：

- 本轮及后续检索请求一律**不做向量化**：笔记检索走既有的"全量返回笔记"，
  Tool RAG 走既有的"全量下发工具"。已入库的旧向量保持不动，不删除、不污染。
- 熔断时向前端发一次告警事件（同一熔断周期内只发一次，避免刷屏），文案要点：
  哪个模型、失败原因、以及"笔记语义检索已临时停用"。
- 熔断态的清除时机严格限定为三个：重启程序、保存云端供应商配置（含密钥/地址/模型名任一变化）、
  切换本地/云端开关。清除后按新的后端命名空间重新向量化并入库（保存配置后自动后台预热，见步骤 6）。
- 熔断不改变 `namespace()`：仍在云端命名空间下，避免半路切到本地空间导致同库混维度。

不做"自动改用本地继续向量化"。那样要同时维护两份索引（云端维度一份、本地 512 维一份），
每次检索都可能触发另一侧全量重建，复杂度与存储都翻倍；用户明确要求的是失败即跳过向量化。

## 5. 分步实施

### 步骤 1 · 抽象接口（notes）

- `embedding/mod.rs` 新增 `Embedder` trait；`EmbeddingEngine` 实现它。
- `retrieval::search` 与 `ToolRag` 的参数/字段类型改为 `&dyn Embedder` / `Arc<dyn Embedder>`。
- `state.rs` 的 `embedding` 字段类型改为 `Arc<dyn Embedder>`。
- 注入点 `src-tauri/src/app/setup/build.rs` 改为构造 Selector（此步先只做本地直通）。
- 验收：`cargo check --workspace` 通过，行为零变化，现有测试全绿。

### 步骤 2 · 维度参数化（notes）

- `vector.rs`：去掉 `EMBEDDING_DIM` 常量，`open(path, dimension)` 建库时固定该库维度，
  写表 DDL 用 `FLOAT[{dim}]`，`vector_to_blob` 按实例维度校验；`VectorError::Dimension`
  的 expected 取实例维度。
- `EmbeddingEngine` 里 512 的自检放宽为「与声明维度不一致只记 debug 日志」，不失败。
- 维度尚未确定时（云端首次调用前）不建库：`namespace()` 可先返回待定向，检索走降级。
- 验收：`cargo test -p ling-chat-notes` 全绿（现有维度断言需按实例维度改写）。

### 步骤 3 · 云端后端与路由（main）

- `ai_service/embedding_remote.rs`（或 `ai_service/llm/` 旁）：`RemoteEmbedder`，
  用 genai 构造 client，复用 `providers/genai_provider.rs` 里现成的 provider 协议映射与
  `normalize_base_url`；`embed_batch` 内部 `Handle::block_on`，带 5 次重试与超时。
  首次响应确定维度并缓存。
- `EmbeddingSelector`：读 `ToolSettings.embedding`，未就绪→本地，就绪→云端；
  持有熔断标记与告警发送。
- 保存配置的热重载路径里重建 Selector，并让 Tool RAG 的内存向量缓存按后端标识作废
  （否则新旧维度混在同一份缓存里打分）。
- 新增 IPC `test_embedding`：嵌入一句固定文本，回报生效后端、维度、耗时，供设置页自检。
- 新增 IPC `rebuild_embeddings`：后台预热命令。逐角色读取笔记全量嵌入写库，同时把
  Tool RAG 的工具示例句嵌入预热进缓存；通过事件回报进度（已完成/总数/失败数），
  可取消。预热期间检索照常走当前可用后端，不阻塞对话。
- 验收：`cargo check --workspace`；`cargo test -p ling-chat-main --lib embedding`；
  用真实端点跑通一次 `test_embedding` 与一次完整预热。

### 步骤 4 · 配置模型（main）

`ToolSettings` 增加 `embedding` 段（`#[serde(default)]`，字段 snake_case，与前端 TS 对齐）：

| 字段            | 说明                                                     |
| --------------- | -------------------------------------------------------- |
| `cloud_enabled` | 本地/云端切换开关，即 UI 上"使用云端向量化模型"          |
| `provider`      | `"openai"`（OpenAI 兼容，含第三方 base_url）/ `"gemini"` |
| `base_url`      | 兼容端点，留空走官方                                     |
| `api_key`       | 密钥                                                     |
| `model`         | 模型名，如 `text-embedding-3-small`、`bge-m3`            |
| `timeout_secs`  | 单次调用超时                                             |

`is_ready()`：`cloud_enabled` 且 `model` 非空且（`provider` 为 gemini/openai 官方时 key 非空；
自定义 base_url 时允许 key 为空，兼容本地自托管）。默认值全部指向本地态。

**独立配置，不复用大模型管理的 provider。** 理由：常见组合是"对话用 A 家、向量化用 B 家"
（例如对话在 DeepSeek、向量在硅基流动），复用 provider 会让这种组合无解。
代价仅是用户多填一次密钥，可接受。

### 步骤 6 · 后台预热（main）

保存 `ToolSettings` 时，若 `embedding` 段发生了影响向量的变化（`cloud_enabled` 翻转、
`provider` / `base_url` / `model` 任一变化），自动在后台启动一次预热：

- 逐角色读笔记全量、分批嵌入、写进该后端的命名空间库；Tool RAG 的示例句同步预热。
- 每批之间检查取消信号；整体带并发上限，避免打满端点或被限流。
- 通过事件回报进度：后端标识、已完成条目数、总数、失败数。
- 预热期间与预热失败后，检索继续按当前可用后端工作；预热不改变熔断语义——
  若预热中连续失败触发熔断，立即停止并告警。
- 同一时刻只允许一个预热任务；新的一次保存会取消上一个（配置已被覆盖）。

注意预热是"用额度换体验"的动作：即使用户此后不再用笔记功能，这批调用也已发生。
因此只在配置**确实影响向量**时才触发，单纯改超时或改无关工具设置不预热。

### 步骤 5 · 前端（可与步骤 3/4 并行）

落点：工具设置 → 左侧「笔记功能」（`memory` 分组页）。现在这一页只走通用分支
（`SettingsTools.vue` 末尾 `v-else`，一个开关），需要给它单独的 `v-else-if="selected === 'memory'"` 区块：

1. 第一行保持现有 `groups.memory` 开关（笔记功能总开关）。
2. 总开关 ON 时，其下显示第二个开关：**"使用云端向量化模型（可以更聪明）"**。
   这个开关同时是本地↔云端的切换器，绑定 `embedding.cloud_enabled`。
3. 该开关 ON 时再展开供应商配置：provider 下拉（OpenAI 兼容 / Gemini）、
   模型名、Base URL、API Key、超时，以及"测试连接"按钮显示生效后端与维度。
4. 隐私告知：开启云端开关时就地显示一行提示——笔记内容将发送给第三方模型处理。
   不做静默开启。
5. 预热进度：保存配置后面板内显示预热进度条（`已完成/总数`，失败数），完成后自动隐藏。
6. 熔断告警：监听新事件，在主界面提示语义检索已临时停用。
7. `src/api/services/tool-settings.ts` 补 `EmbeddingSettings` 接口与 `ToolSettings.embedding`，
   加 `testEmbedding()`；i18n 四个语言包（en/ja/zh-CN/zh-HK）补 `ui.toolCalls.embedding.*` 文案。

前端注意：保存按钮在页面底部是共用的，点保存会提交整份 `ToolSettings`。
预热只在后端检测到影响向量的字段真的变了才触发，前端不需要额外判断，也不要自己发起预热。

## 6. 执行顺序

按"后端先行、校验挂后台"的既定习惯：步骤 1→2→3→4→6 连续写完 Rust，`cargo check --workspace`
与 `cargo test --workspace` 放后台跑，期间并行做步骤 5 的前端与文案，最后 `pnpm build`
收口，再清一遍注释与多余测试代码。

真实端点联调已有条件（用户可提供 embeddings 端点）。联调清单：
`test_embedding` 单条调用 → 确认维度与耗时 → 触发一次完整预热 → 确认进度事件与入库数量
→ 断开网络验证 5 次重试与熔断告警 → 恢复后确认熔断仍在、重启后恢复正常。

## 7. 风险

- 维度是这次改动唯一的破坏性风险：向量库文件按维度建表，误用会直接报
  `Dimension` 错误而不是静默错乱，可接受。切云端后本地旧库不删（留在原目录），
  切回来即复用，无迁移成本。
- 预热是网络密集的批量调用：首次切到云端会一次性嵌入全部角色的全部笔记，
  需要控制批大小、并发上限与超时，并保证用户改配置时能取消上一个任务，
  否则会既浪费额度又持续占用端点。预热失败不应把整次保存判为失败。
- Tool RAG 的示例句目前只在内存缓存里，预热只对它做"提前算好"，进程重启即失效。
  等工具侧真的把向量落库后，这一步可以省掉；在那之前不必为它引入持久化。
- 熔断标记是进程内状态，不持久化；重启即重置（符合已定决策）。
- 云端配置里 `is_enabled()` 为 false 时 Tool RAG 与笔记检索都退回全量下发/返回，
  对话主流程不受影响，但 token 消耗会上升，值得在告警文案里体现。

## 8. 附带收益

移动端（Android/iOS）目前因为不带 ONNX Runtime 与模型文件，语义检索完全禁用。
云端后端不依赖 ONNX，落地后移动端可以第一次真正用上笔记语义检索。
