# 角色压缩包导入 / 导出

> 跨端角色分发能力：用户把角色文件夹打成 `.zip` / `.7z` 压缩包，在桌面或 Android 上相互迁移。
> 源数据零丢失、并发安全、可中途取消。

## 1. 用户故事

- 桌面端：设置页 → "从压缩包导入" 选择 `.zip` / `.7z`，导入完成后角色出现在列表中；角色卡片菜单 → "导出为 .zip / .7z" 选目标位置保存。
- Android 端：设置页 → "从压缩包导入" 调起 SAF 选择压缩包；角色卡片菜单 → "导出" 调起 SAF `ACTION_CREATE_DOCUMENT` 让用户选保存位置。

## 2. 总体数据流

### 2.1 导入

```
[用户]               [前端]                            [后端 / Rust]
点击导入  ─►  pickAndImport(file)
                       │
                       │  invoke('import_role_from_path',
                       │          { path, format, conflict, fileName })
                       ▼
                                         1. 并发闸 (RoleArchiveState.importing)
                                         2. 生成 task_id + CancellationToken
                                         3. emit('role:import-started', { task_id })
                                         4. prepare_import_source(path)
                                            ├─ 普通路径   → 走 file 系统读取
                                            └─ content:// → 复制到 cache/imports/
                                         5. detect_format + extract_zip/extract_sevenz
                                            (每条目 emit role:import-progress)
                                         6. 落盘到 data/game_data/characters/<name>/
                                         7. DB 写入/更新 role 记录
                                         8. rescan_roles 重新同步索引
                                         9. emit('role:list-updated')
                       │
                       │  监听 role:import-progress 更新 store
                       │  监听 role:import-error  标记 phase=error
                       ▼
                  [ImportProgressBar UI]
```

### 2.2 导出

```
[用户]               [前端]                            [后端 / Rust]
点 ⋮ 选 .zip ─►  saveDialog({ defaultPath, filters })
                       │
                       │  用户选完返回 destPath（普通路径或 content://）
                       │  invoke('export_role_to_path',
                       │          { roleId, format, destPath })
                       ▼
                                         1. 查 DB 拿 role.resource_folder
                                         2. compress(src_dir, format, temp)
                                            (每条目 emit role:import-progress)
                                         3. 拷贝到 destPath
                                            ├─ 普通路径   → tokio::fs::copy
                                            └─ content:// → android-fs 写 SAF
                                         4. 删除 temp
                       │
                       ▼
                  store.export.phase = 'done'
```

## 3. Tauri 命令清单

注册位置：`src-tauri/src/lib.rs:488-493`，由 `RoleArchiveState` 注入。

| 命令 | 入口 | 说明 |
|---|---|---|
| `import_role` | `mod.rs:43` | 接受 `bytes: Vec<u8>`。保留给已把压缩包读进内存的旧调用方使用，**不推荐**新代码使用。 |
| `import_role_from_path` | `mod.rs:138` | **推荐入口**。`path` 接受桌面文件路径或 Android `content://` URI。 |
| `cancel_role_import` | `mod.rs:109` | 必传 `taskId`；后端从 `state.tasks` 摘掉对应 entry 并触发 `CancellationToken`。 |
| `export_role` | `mod.rs:234` | 压缩到 cache 返回 `{ temp_path, suggested_name, size_bytes }`，由前端自行落地。 |
| `export_role_to_path` | `mod.rs:249` | 压缩并直接写入 `destPath`；跨端统一（桌面原生 copy / Android android-fs）。**推荐入口**。 |
| `rescan_roles` | `mod.rs:332` | 重扫 `data/game_data/characters/` 并 emit `role:list-updated`。 |

## 4. 事件

| 事件 | 触发时机 | Payload | 订阅方 |
|---|---|---|---|
| `role:import-started` | 后端分配 `task_id` 后立即 | `RoleImportStartedEvent { task_id }` | `useRoleImportExport` 缓存到 `currentTaskId` |
| `role:import-progress` | 每写入一个 entry（throttled 50ms / 100 entry） | `EntryEvent` | `ImportProgressBar` |
| `role:import-error` | 流程失败 | `string` | `useRoleImportExport` |
| `role:list-updated` | `import_role` / `rescan_roles` 成功 | `()` | 角色列表 store |

## 5. 前端架构

### 5.1 文件清单

- `src/api/services/role-archive.ts` — `invoke` 包装 + 类型导出
- `src/composables/useRoleImportExport.ts` — 调度（pickAndImport / runImport / cancel / doExport / rescan）
- `src/stores/modules/ui/role-archive.ts` — 全局进度状态（`store.import` + `store.export`）
- `src/components/ui/ImportProgressBar.vue` — 进度条 UI，`<Teleport to="body">` 全局可见
- `src/components/ui/RoleArchiveProgress.vue` — 仅 `<ImportProgressBar />` 包装，`SettingsCharacter` 入口处挂载

### 5.2 状态机

```
         ┌──── cancel() ────┐
         ▼                  │
   ┌─► running ─────────► done
   │     │
idle─┤     └─► error ─► (dismiss)
   │     │
   │     └─► cancelled
   │
   └─► running (export) ─► done
                  └─► error
```

字段：`phase` / `percent`（-1 表示 indeterminate）/ `message` / `error` / `fileName` / `format` / `startedAt` / `savedPath`。

### 5.3 取消流程

1. UI 点 "取消" → `cancel()`。
2. `useRoleImportExport` 用 `currentTaskId` 调 `cancelRoleImport(taskId)`。
3. 后端 `state.tasks.remove(&task_id)` → `cancel_token.cancel()` → 立即清理 SAF 缓存。
4. 解压循环下次 `is_cancelled()` 检查返回 `Err(ArchiveError::Cancelled)`。
5. 前端 `store.import.phase = 'cancelled'`，2.5s 后自动消失。

## 6. 后端架构

### 6.1 模块拆分（`src-tauri/src/api/role_archive/`）

| 文件 | 职责 |
|---|---|
| `mod.rs` | 5 个 Tauri 命令、公开类型 `ImportResult` / `ExportResult`、再导出 |
| `state.rs` | `RoleArchiveState`（`tasks: HashMap<task_id, ImportTaskEntry>` + `importing: AtomicBool`）、`ImportingGuard` / `TaskRemoveGuard` |
| `import_pipeline.rs` | `do_import`、`write_temp_archive`、`prepare_import_source`、`locate_extracted_dir`、`sanitize_role_folder_name`、`copy_dir_recursive`、`find_role_id_by_folder`、`parse_format`、`parse_policy` |
| `export_pipeline.rs` | `compress_role_to_temp`、`sanitize_file_name` |

### 6.2 归档工具（`src-tauri/src/utils/archive/`）

| 文件 | 职责 |
|---|---|
| `mod.rs` | 公共类型 `ArchiveFormat` / `ConflictPolicy` / `ArchiveError` / `EntryEvent` / `ExtractSummary` / `TargetResolution`、公开 API 再导出 |
| `safety.rs` | `check_entry_safety`（条数 + 压缩比）、`sanitize_entry_name`、`safe_join`（防 zip-slip） |
| `resolve.rs` | `resolve_target`（冲突策略 → 最终目标路径） |
| `extract.rs` | `extract_zip` / `extract_sevenz`，消费 `CancellationToken` |
| `compress.rs` | `compress`（zip / 7z 入口），跳过 macOS metadata |

## 7. 并发与取消

- **全局单实例导入**：`RoleArchiveState.importing` 是 `AtomicBool`，第二次 `swap` 失败直接 `Err("已有导入任务在进行中")`。
- **任务隔离**：每次导入生成独立 `task_id`（UUID v4）和 `CancellationToken`；互不影响。
- **资源清理**：`TaskRemoveGuard` 在 `Drop` 时把 `task_id` 从 `state.tasks` 摘掉，避免泄漏。
- **取消感知**：解压循环每条 entry 后检查 `cancel_token.is_cancelled()`；throttle 50ms / 100 entry 一次，IPC 不刷屏。
- **取消不撤回已写入文件**：取消语义是 "停止后续写入"，已落盘的 partial 目录由用户决定是否删除。

## 8. 安全策略

| 风险 | 防御位置 | 行为 |
|---|---|---|
| zip-slip（`../`、绝对路径、Windows UNC） | `sanitize_entry_name` + `safe_join` | 拒绝写入，抛 `PathTraversal` |
| macOS metadata（`__MACOSX/`、`._*`、`.DS_Store`） | `sanitize_entry_name` | 跳过 entry，记 warning |
| 单归档条目过多 | `check_entry_safety` | `> MAX_ENTRY_COUNT (1000)` 抛 `TooManyEntries` |
| 单条解压炸弹（已解压 / 压缩 > 100） | `check_entry_safety` | 抛 `CompressionRatio` |
| 加密压缩包 | `ArchiveError::PasswordProtected` | 提示用户先在外部解密 |
| 控制字符文件名 | `sanitize_role_folder_name` | 替换非法字符为 `_`，保留字母数字 + `-` + `.` |
| 冲突目录 | `resolve_target` + `ConflictPolicy` | `skip` / `rename` / `overwrite` 三策略 |

## 9. 错误处理

`ArchiveError` 变体：

- `UnsupportedFormat(String)` — 文件头魔数不匹配
- `TooManyEntries(usize)` — 条目数超限
- `CompressionRatio { actual, compressed }` — 单条压缩比超 100
- `PathTraversal(String)` — zip-slip 拒绝
- `InvalidName(String)` — 文件名清洗失败
- `Zip(String)` / `SevenZ(String)` — 底层 crate 错误
- `Io(io::Error)` — IO 错误
- `PasswordProtected` — 加密压缩包
- `Cancelled` — 用户取消
- `AlreadyExists(String)` — `ConflictPolicy::Skip` 命中

Tauri 命令层把所有错误转成 `String`，前端通过 `invoke` 的 `Promise.reject` 接收。

## 10. 关键代码位置

| 文件 | 用途 |
|---|---|
| `src-tauri/src/api/role_archive/mod.rs` | 5 个 Tauri 命令 + 类型 |
| `src-tauri/src/api/role_archive/state.rs` | 全局状态与守卫 |
| `src-tauri/src/api/role_archive/import_pipeline.rs` | 导入流水线 |
| `src-tauri/src/api/role_archive/export_pipeline.rs` | 导出流水线 |
| `src-tauri/src/utils/archive/mod.rs` | 公共类型与 API |
| `src-tauri/src/utils/archive/safety.rs` | 路径与压缩比安全 |
| `src-tauri/src/utils/archive/extract.rs` | zip / 7z 解压 |
| `src-tauri/src/utils/archive/compress.rs` | zip / 7z 压缩 |
| `src-tauri/src/utils/archive/resolve.rs` | 冲突策略 |
| `src/composables/useRoleImportExport.ts` | 前端调度 |
| `src/stores/modules/ui/role-archive.ts` | 进度状态 store |
| `src/components/ui/ImportProgressBar.vue` | 进度条 UI |
| `src/components/ui/RoleArchiveProgress.vue` | 进度条挂载点 |
| `src/api/services/role-archive.ts` | invoke 封装 |

## 11. 已知约束

- 暂存目录用 `cache/imports/` 和 `cache/exports/`，受 OS 临时空间限制。
- SAF `content://` URI 仅在 Android 由前端传过来；桌面端用普通绝对路径。
- 加密压缩包需要用户在外部先解压（暂不支持带密码解压）。
- 取消导入不会撤回已写入的角色目录文件；partial 目录需要用户手动清理。
- `role:import-progress` 节流到 50ms / 100 entry 一次，避免 IPC 风暴。
- 跨端保存路径：桌面走 `plugin-dialog.save` + 原生拷贝，Android 走 SAF。

## 12. 调试建议

- Rust 端：`RUST_LOG=ling_chat::api::role_archive=debug,ling_chat::utils::archive=debug`。
- 前端：DevTools → Tauri 标签，监听 `role:import-started` / `role:import-progress` / `role:import-error` / `role:list-updated`。
- `cancelRoleImport` 失败的常见原因：`taskId` 为空（当前没有进行中的任务，或 `role:import-started` 事件还没到）。
- 角色目录没出现但进度条显示成功：检查 `data/game_data/characters/<name>/settings.yml` 是否存在；不存在说明归档里没有 `settings.yml`，被识别为非角色压缩包。