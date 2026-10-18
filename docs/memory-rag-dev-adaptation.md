# 长期记忆 RAG 的 dev 适配与验证

日期：2026-10-18。适配基线为 `upstream/dev` 的 `4e3f37b5031b40fe2001b4a102d9f67c6b68d06d`。

## 适配方式

在最新 dev 基线上重新应用 P0–P5 改动，解决 11 个文件的合并冲突。旧阶段文档中的源码位置与命令如有差异，以本记录和当前源码为准。

| 部分         | 最终处理                                                                                                         |
| ------------ | ---------------------------------------------------------------------------------------------------------------- |
| 模型引擎     | 保留上游迁移到 notes 的结构、宿主传入 ONNX 可用性的接口和批量推理；补入无截断 token 计数及模型、分词器版本指纹   |
| 笔记检索     | 保留上游 `retrieval::NotesVectorIndex`、sqlite-vec、`memory_get_notes` 参数与纯向量排序                          |
| 长期记忆检索 | 混合排序置于独立的 `memory_retrieval`，继续使用带来源和版本的宿主 SQLite 索引，供自动回忆与 `memory_search` 共用 |
| 工具推荐     | 保留上游 Agent / RAG 推荐、相关性阈值、激活和权限求交，接入记忆检索会话                                          |
| 聊天队列     | 同时保留上游回合结束截断与 RAG 来源快照，工具后续写重新开始边界检测，旧来源回复不能改标为新来源                  |
| 上下文重建   | 修改笔记后清除旧检索正文，保留工具结果配对、当轮推荐、输入图片、工具图片及后台通知；临时图片不回填长期历史       |
| 依赖         | 沿用 dev 的 Rust / 前端依赖版本，Cargo.lock 仅补入 notes 对 sha2 的依赖                                          |

Tool RAG、笔记语义检索与长期记忆检索继续共享同一个 EmbeddingEngine。两种检索各自保留适用的数据范围和排序方式，笔记工具不会被替换成聊天混合检索。

## 回归覆盖

- 保留 P0–P5 的来源、存档分支、角色感知、索引生命周期、缓存、工具配对和自动保存协调测试。
- 扩充真实 producer 队列测试：旧上下文产生越界玩家文本，再更新笔记；越界文本被截断，旧回复被拒绝，新上下文回复仍正常发布。
- 扩充笔记修改后的上下文重建测试：推荐提示和工具图片保留，旧检索正文清除，已执行、失败和未执行的工具结果仍保持完整配对。
- 扩充会话基础上下文测试：当轮推荐、输入图片及后台通知在引用作废后保留。
- 将依赖宿主数据库的真实分块与重启复用测试留在 main，notes 的真实批量嵌入测试继续独立存在，维持 main → notes 依赖方向。

## 验证环境

前端依赖按 dev 锁文件重新安装，使用 pnpm 12.14.0 构建，没有修改 package.json 或 pnpm-lock.yaml。

首次离线 Rust 检查缺少 dev 更新后的依赖，联网补齐后继续使用 locked / offline 验证。复跑期间磁盘空间不足，通过 Cargo 清理本项目业务包的旧编译产物后重新构建，没有清理源代码、模型、角色设置或聊天数据。

Windows 上单独测试 notes 时，既有 rusqlite 配置需要系统 sqlite3.lib；本次使用工作区测试，让宿主既有的 bundled SQLite 特性统一生效，没有为适配额外改变上游 SQLite 配置。

Node 默认测试子进程出现数据反序列化错误，模型下载脚本测试改用 `--test-isolation=none` 后执行完整 10 项用例。

## 复现命令

本轮最终结果：

| 检查                   | 结果                                                                                                       |
| ---------------------- | ---------------------------------------------------------------------------------------------------------- |
| 工作区测试             | 235 项通过、0 失败，另有 9 项默认忽略；main 114、数据库集成 58、notes 47、plugins 16，桌面壳与文档测试通过 |
| 工作区编译检查         | locked / offline 通过                                                                                      |
| 前端类型检查与生产构建 | pnpm build 通过；保留既有大分包提示                                                                        |
| 模型下载脚本           | 10 项通过、0 失败                                                                                          |
| 真实记忆检索校准集     | 有答案 10/10；无答案误召回 0/2；补充相近负例误召回 0/4；热缓存 12/12                                       |
| 两组真实记忆检索保留集 | 各有答案 5/5、无答案误召回 0/1、热缓存 6/6                                                                 |
| 真实聊天宿主           | 窗口外银色书签召回，向量 ready；1 条、838 字符；热缓存命中，原历史不变                                     |
| 真实分块与索引重启     | 2 个文档、4 个分块，重开数据库复用全部 4 个向量                                                            |
| 上游笔记检索与批量嵌入 | 两项真实 ONNX 测试分别通过                                                                                 |

有答案 hit@5 达标不代表全部附带片段都相关；既有 q18 弱相关老师片段仍是质量限制。以上均使用本地模型与隔离测试数据，没有调用付费聊天接口；不扩大为桌面端到端或安装包验收结论。

常规检查：

```powershell
cargo test --workspace --locked --offline --quiet
cargo check --workspace --locked --offline --quiet
pnpm build
node --test --test-isolation=none scripts/download_embedding_model.test.mjs
```

真实模型测试每条命令分别启动进程，避免在同一个进程重复初始化 ONNX Runtime：

```powershell
cargo test --workspace --lib --locked --offline memory_search_real_calibration -- --ignored --nocapture
cargo test --workspace --lib --locked --offline memory_search_real_holdout -- --ignored --nocapture
cargo test --workspace --lib --locked --offline memory_search_real_fresh_holdout -- --ignored --nocapture
cargo test --workspace --lib --locked --offline memory_recall_real_host -- --ignored --nocapture
cargo test --workspace --lib --locked --offline memory_embedding_real_index_restart -- --ignored --nocapture
cargo test --workspace --lib --locked --offline retrieval::tests::retrieval_hits_semantic_notes -- --ignored --nocapture
cargo test --workspace --lib --locked --offline embedding_smoke_returns_normalized_semantic_vectors -- --ignored --nocapture
```
