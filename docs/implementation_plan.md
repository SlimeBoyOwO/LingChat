# Tauri Resources 打包迁移 — 实施计划

> **版本**: v2（基于最终方案讨论）
> **日期**: 2026-07-05
>
> **目标**: 游戏资源通过 Tauri `bundle.resources` 打入安装包，安装到 `data/.official/`。首次启动全量 seed → 删除 `.official/`。App 更新后 `.official/` 重新出现 → 用户在设置页查看差异并选择性同步 → 删除 `.official/`。删除基于网络下载的 `data_update` 模块，统一为单层更新体系。
>
> **原则**:
> - 只有 exe + data/，不引入额外顶层目录
> - 不影响 Android 版本
> - 不影响 LAN Sync 功能
> - 用户自定义文件永远不被触碰
> - 不做文件删除（diff 仅含 add + modify）

---

## 目录

1. [最终架构](#1-最终架构)
2. [核心机制：.official 种子目录](#2-核心机制official-种子目录)
3. [模块化：manifest 公共类型](#3-模块化manifest-公共类型)
4. [实施步骤](#4-实施步骤)
5. [文件变更清单](#5-文件变更清单)
6. [验证清单](#6-验证清单)

---

## 1. 最终架构

### 1.1 安装后的目录结构

```
安装目录 (NSIS per-user, 如 %LOCALAPPDATA%\Programs\LingChat\)
═══════════════════════════════════════════
LingChat.exe
data/                                    ← resolve_data_dir() 返回这里
  .official/                             ← bundle.resources 释放（种子源）
    game_data/                           ← 安装包中的默认资源
      backgrounds/
      characters/
      musics/
      scripts/
    data_manifest.json                   ← resource 版本号 + 文件清单
  third_party/                           ← bundle.resources 直接释放（不走 .official）
    emotion_model_19emo/
      model.onnx
      vocab.txt
      ...
```

**关键点**:
- `.official/` 是隐藏目录（`.` 前缀），用户日常不感知
- `third_party/` 直接释放到 `data/third_party/`，不走种子机制（installer 直接管理，更新时直接覆盖）
- `data/` 下其他内容（`game_data/`、`data_manifest.json`）**在首次 seed 后才出现**

### 1.2 首次启动（seed）

```
条件: data/.seeded 不存在

1. 检查 data/.official/ 是否存在
   ├─ 不存在 → 跳过（开发环境或损坏）
   └─ 存在 →
        a. 递归复制 data/.official/game_data/* → data/game_data/
        b. 复制 data/.official/data_manifest.json → data/data_manifest.json
        c. 写入 data/.seeded（标记文件，内容为空）
        d. 递归删除 data/.official/
```

### 1.3 正常运行

```
条件: data/.seeded 存在 && data/.official/ 不存在

→ 什么都不做，data/ 中已有工作副本
```

### 1.4 App 更新后

```
条件: data/.seeded 存在 && data/.official/ 存在（installer 重新释放）

1. 用户打开设置 → 看到"数据版本"有新版本可更新
2. 点击 → ResourceSyncDialog 打开
3. 后端: 比对 .official/data_manifest.json vs data/data_manifest.json
4. 前端: 展示文件差异列表（新增 + 修改），用户勾选
5. 用户点击"同步" → 选中文件从 .official/ 复制到 data/（原子 .tmp→rename）
6. 更新 data/data_manifest.json
7. 删除 data/.official/
```

### 1.5 数据流

```
[CI 构建]                              [用户机器]
─────────                              ─────────
git ls-files data/                    首次安装:
  ↓                                    installer → data/.official/game_data/
generate-data-manifest.js              installer → data/third_party/
  ↓                                               data/.official/data_manifest.json
data_manifest.json                     
  ↓                                    首次启动:
tauri build (bundle.resources)           seed: .official/ → data/game_data/
  ↓                                               .official/ → data/data_manifest.json
NSIS installer (含 .official/ +          delete .official/
              third_party/)              
                                       更新后:
                                         installer → 覆盖 exe
                                         installer → 重新创建 .official/
                                         installer → 覆盖 third_party/
                                         用户 → 设置页查看 diff → 选择性同步
                                         delete .official/

更新检查（仅一种）:
  check() [tauri-plugin-updater] → latest.json → app update?
```

---

## 2. 核心机制：.official 种子目录

### 2.1 状态机

```
                  ┌──────────────────────────────┐
                  │       首次安装后               │
                  │  .official/ ✓                │
                  │  .seeded    ✗                │
                  └──────────────┬───────────────┘
                                 │
                          seed_data_dir()
                                 │
                  ┌──────────────▼───────────────┐
                  │       日常使用                 │
                  │  .official/ ✗ (已删除)        │
                  │  .seeded    ✓                │
                  └──────────────┬───────────────┘
                                 │
                         App 更新 (installer)
                                 │
                  ┌──────────────▼───────────────┐
                  │       更新待同步               │
                  │  .official/ ✓ (重新出现)      │
                  │  .seeded    ✓                │
                  └──────────────┬───────────────┘
                                 │
                     用户点击"同步数据"
                                 │
                  ┌──────────────▼───────────────┐
                  │       日常使用                 │
                  │  .official/ ✗ (已删除)        │
                  │  .seeded    ✓                │
                  └──────────────────────────────┘
```

### 2.2 判断逻辑

```rust
// seed_data_dir() 伪代码
fn seed_data_dir(app: &AppHandle) -> Result<()> {
    let data_dir = get_data_dir();
    let seeded = data_dir.join(".seeded");
    let official = data_dir.join(".official");

    // 桌面端逻辑
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        if !official.exists() {
            // .official 不存在 → 无需处理
            return Ok(());
        }

        if !seeded.exists() {
            // 首次启动：全量 seed
            seed_full_from_official(&data_dir, &official)?;
            write_seeded(&seeded)?;
            remove_official(&official)?;
        }
        // seeded 存在 + official 存在 → 更新场景
        // 不做自动操作，等用户手动触发 sync
    }

    // Android 逻辑不变
    #[cfg(any(target_os = "android", target_os = "ios"))]
    { /* 保持现有 data.zip 解压逻辑 */ }

    Ok(())
}
```

### 2.3 为什么 third_party 不走 .official

- `third_party/emotion_model_19emo/model.onnx` ~390MB
- 用户**不会修改**模型文件
- App 更新时 installer 直接覆盖，用户无需手动同步
- 避免 seed 时复制 390MB 的大文件（浪费时间）

---

## 3. 模块化：manifest 公共类型

### 3.1 问题

`DataManifest` 和 `FileEntry` 目前定义在 `data_update::manifest.rs` 中。该模块要被删除，但类型被以下模块依赖：

| 使用方 | 文件 |
|---|---|
| `lan_sync::manifest` | diff + CompleteManifest 构建 |
| `lan_sync::server` | /manifest 端点 |
| `lan_sync::messages` | CompleteManifest 序列化 |
| `resource_sync::sync` (新) | 比对 + 复制 |

### 3.2 方案

```
src-tauri/src/manifest/           ← 新建：公共模块
  mod.rs
    - FileEntry { sha256, size, modified_at }
    - DataManifest { data_version, files: HashMap }
    - ManifestDiff { files_to_add, files_to_modify, files_to_remove }
    - impl DataManifest::diff(&self, other) -> ManifestDiff
    - impl DataManifest::load(path) -> Result<Self>
    - impl DataManifest::save(&self, path) -> Result<()>
    - fn compute_sha256(path) -> String

src-tauri/src/resource_sync/      ← 新建：资源同步
  mod.rs    → Tauri commands
  sync.rs   → seed_full_from_official(), apply_selected_files()

src-tauri/src/lan_sync/           ← 修改：更新 use 路径
  manifest.rs   use crate::manifest::{...}
  server.rs
  messages.rs
```

**不修改 `lan_sync::manifest::diff_manifests()` 的逻辑**。它使用 `CompleteManifest`（含 `runtime_files` + `modified_at` 冲突解决），与 resource_sync 的简单 `DataManifest::diff()` 职责不同。

---

## 4. 实施步骤

### Step 1: 提取公共 manifest 模块

**文件操作**:

1. **新建** `src-tauri/src/manifest/mod.rs`:
   - 从 `data_update::manifest.rs` 移入 `FileEntry`、`DataManifest`
   - 添加 `ManifestDiff` 结构体
   - 添加 `DataManifest::diff()` — 纯集合运算
   - 添加 `DataManifest::load()` / `DataManifest::save()` — 文件读写 + 原子写入
   - 从 `data_update::sync.rs` 移入 `compute_sha256()`

2. **修改** `src-tauri/src/lib.rs`:
   ```rust
   mod manifest;
   ```

3. **修改** `src-tauri/src/lan_sync/` 所有引用:
   ```
   use crate::data_update::manifest::*  →  use crate::manifest::*
   ```

4. **修改** `src-tauri/src/data_update/manifest.rs`:
   ```rust
   // 改为 re-export，过渡期向后兼容
   pub use crate::manifest::*;
   ```

5. `cargo check` 验证通过

### Step 2: 配置 Tauri bundle.resources + 构建脚本

#### 2.1 tauri.conf.json

```jsonc
{
  "build": {
    "beforeBuildCommand": "node scripts/generate-data-manifest.js --output data/data_manifest.json && pnpm build",
    // ...
  },
  "bundle": {
    "active": true,
    "targets": "nsis",    // 仅 NSIS per-user（移除 msi）
    "icon": ["icons/icon.ico"],
    "createUpdaterArtifacts": true,
    "resources": {
      "data/game_data/":              "data/.official/game_data/",
      "data/third_party/":            "data/third_party/",
      "data/data_manifest.json":      "data/.official/data_manifest.json"
    }
  }
}
```

**映射说明**:

| 源路径（项目） | 目标路径（安装目录下） | 说明 |
|---|---|---|
| `data/game_data/` | `data/.official/game_data/` | 默认资源，隐藏目录 |
| `data/third_party/` | `data/third_party/` | 模型文件，直接替换 |
| `data/data_manifest.json` | `data/.official/data_manifest.json` | 版本清单 |

#### 2.2 generate-data-manifest.js

修改：
- 读取 `process.env.DATA_VERSION`（CI 注入 `github.run_number`），默认 `1`
- **新增排除** `third_party/`（不进入 desktop manifest，因为由 installer 直接管理）
- 输出路径由 `--output` 参数控制
- 其他逻辑不变（`git ls-files` + SHA-256 + 过滤排除项）

#### 2.3 capabilities/default.json

确认已有权限：
```json
"updater:default",
"updater:allow-check",
"updater:allow-download-and-install",
"process:allow-restart"
```

无需新增。（resource_sync 走 `invoke`，不经过 plugin permission 系统）

### Step 3: 扩展桌面端 seed 逻辑

**文件**: `src-tauri/src/init/static_copy.rs`

**核心变更**: `seed_data_dir()` 桌面端从 no-op 变为实际逻辑。

```rust
pub fn seed_data_dir(app: &tauri::AppHandle) -> anyhow::Result<()> {
    let data_dir = get_data_dir().clone();

    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        // 现有的移动端逻辑，不变
        let marker = data_dir.join(".seeded");
        let manifest = data_dir.join("data_manifest.json");
        if marker.exists() && manifest.exists() { return Ok(()); }
        let _ = std::fs::remove_file(&marker);
        seed_via_fs_plugin(app, &data_dir)?;
        std::fs::write(&marker, b"")?;
        tracing::info!("Data directory seeding complete (mobile)");
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        seed_desktop(app, &data_dir)?;
    }

    Ok(())
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn seed_desktop(app: &tauri::AppHandle, data_dir: &Path) -> anyhow::Result<()> {
    let official = data_dir.join(".official");
    let seeded = data_dir.join(".seeded");

    // .official 不存在 → 无需操作
    if !official.exists() {
        return Ok(());
    }

    if !seeded.exists() {
        // 首次启动：全量 seed
        tracing::info!("First launch detected — seeding from .official/");
        crate::resource_sync::sync::seed_full_from_official(data_dir, &official)?;
        std::fs::write(&seeded, b"")?;
        std::fs::remove_dir_all(&official)?;
        tracing::info!("Seed complete, .official removed");
    }
    // seeded 存在 + official 存在 → 更新待同步
    // 不做自动操作，由前端 check_resource_sync 提供 UI

    Ok(())
}
```

**不变**: `get_data_dir()` / `resolve_data_dir()` — 桌面 release 模式仍返回 `exe旁/data/`。

### Step 4: 创建 resource_sync 模块

#### 4.1 `src-tauri/src/resource_sync/mod.rs`

```rust
use std::sync::Mutex;
use serde::Serialize;

/// 防止并发同步的全局状态
#[derive(Default)]
pub struct ResourceSyncState {
    pub syncing: Mutex<bool>,
}

/// check_resource_sync 返回给前端的数据
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceSyncInfo {
    pub available: bool,
    pub new_version: u64,
    pub current_version: u64,
    pub files_to_add: Vec<SyncFileEntry>,
    pub files_to_modify: Vec<SyncFileEntry>,
    pub total_size: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncFileEntry {
    pub path: String,
    pub sha256: String,
    pub size: u64,
    pub change_type: String,  // "add" | "modify"
}

/// apply_resource_sync 返回的结果
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceSyncResult {
    pub success: bool,
    pub files_synced: usize,
    pub message: String,
}

/// 检查数据资源更新
///
/// 对比 data/.official/data_manifest.json 与 data/data_manifest.json，
/// 仅返回 add 和 modify（不做 remove）。
#[tauri::command]
pub async fn check_resource_sync(
    app: tauri::AppHandle,
) -> Result<ResourceSyncInfo, String> {
    let data_dir = crate::init::static_copy::get_data_dir();
    let official_manifest_path = data_dir.join(".official").join("data_manifest.json");
    let local_manifest_path = data_dir.join("data_manifest.json");

    if !official_manifest_path.exists() {
        return Ok(ResourceSyncInfo {
            available: false,
            new_version: 0,
            current_version: 0,
            files_to_add: vec![],
            files_to_modify: vec![],
            total_size: 0,
        });
    }

    let official_manifest = crate::manifest::DataManifest::load(&official_manifest_path)
        .map_err(|e| format!("读取官方清单失败: {}", e))?;

    let local_manifest = if local_manifest_path.exists() {
        crate::manifest::DataManifest::load(&local_manifest_path)
            .map_err(|e| format!("读取本地清单失败: {}", e))?
    } else {
        // 本地无清单 → 视为版本 0，全部文件为"新增"
        crate::manifest::DataManifest {
            data_version: 0,
            files: HashMap::new(),
        }
    };

    if official_manifest.data_version <= local_manifest.data_version {
        // 版本号没有更新, 但可能因为某种原因 .official 残留
        // 清理之
        let _ = std::fs::remove_dir_all(data_dir.join(".official"));
        return Ok(ResourceSyncInfo {
            available: false,
            new_version: official_manifest.data_version,
            current_version: local_manifest.data_version,
            files_to_add: vec![],
            files_to_modify: vec![],
            total_size: 0,
        });
    }

    let diff = local_manifest.diff(&official_manifest);

    let files_to_add: Vec<SyncFileEntry> = diff.files_to_add.iter().map(|path| {
        let entry = &official_manifest.files[path];
        SyncFileEntry {
            path: path.clone(),
            sha256: entry.sha256.clone(),
            size: entry.size,
            change_type: "add".to_string(),
        }
    }).collect();

    let files_to_modify: Vec<SyncFileEntry> = diff.files_to_modify.iter().map(|path| {
        let entry = &official_manifest.files[path];
        SyncFileEntry {
            path: path.clone(),
            sha256: entry.sha256.clone(),
            size: entry.size,
            change_type: "modify".to_string(),
        }
    }).collect();

    let total_size = files_to_add.iter().map(|f| f.size)
        .chain(files_to_modify.iter().map(|f| f.size))
        .sum();

    Ok(ResourceSyncInfo {
        available: !files_to_add.is_empty() || !files_to_modify.is_empty(),
        new_version: official_manifest.data_version,
        current_version: local_manifest.data_version,
        files_to_add,
        files_to_modify,
        total_size,
    })
}

/// 应用选中的文件同步
#[tauri::command]
pub async fn apply_resource_sync(
    app: tauri::AppHandle,
    state: tauri::State<'_, ResourceSyncState>,
    selected_files: Vec<String>,
) -> Result<ResourceSyncResult, String> {
    // 防止并发
    let mut locked = state.syncing.lock().map_err(|e| format!("锁获取失败: {}", e))?;
    if *locked {
        return Err("已有同步进行中".to_string());
    }
    *locked = true;
    drop(locked);

    let result = crate::resource_sync::sync::apply_selected_files(
        &app,
        &selected_files,
    );

    let mut locked = state.syncing.lock().map_err(|_| "锁错误".to_string())?;
    *locked = false;

    result.map_err(|e| e.to_string())
}
```

#### 4.2 `src-tauri/src/resource_sync/sync.rs`

```rust
use std::path::{Path, PathBuf};
use std::collections::HashMap;

/// 首次全量播种: .official/ → data/
pub fn seed_full_from_official(data_dir: &Path, official_dir: &Path) -> anyhow::Result<()> {
    let game_data_src = official_dir.join("game_data");
    let game_data_dst = data_dir.join("game_data");
    let manifest_src = official_dir.join("data_manifest.json");
    let manifest_dst = data_dir.join("data_manifest.json");

    // 1. 复制 game_data/
    if game_data_src.exists() {
        copy_dir_recursive(&game_data_src, &game_data_dst)?;
    }

    // 2. 复制 manifest
    if manifest_src.exists() {
        std::fs::copy(&manifest_src, &manifest_dst)?;
    }

    tracing::info!("Full seed from .official complete");
    Ok(())
}

/// 从 .official/ 复制选中的文件到 data/
pub fn apply_selected_files(app: &tauri::AppHandle, selected_files: &[String]) -> anyhow::Result<ResourceSyncResult> {
    let data_dir = crate::init::static_copy::get_data_dir();
    let official_dir = data_dir.join(".official");

    let official_manifest_path = official_dir.join("data_manifest.json");
    let official_manifest = crate::manifest::DataManifest::load(&official_manifest_path)?;

    let mut synced = 0usize;

    for path in selected_files {
        let src = official_dir.join("game_data").join(path);
        let dst = data_dir.join("game_data").join(path);
        let tmp = data_dir.join(format!("{}.tmp", path));

        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)?;
        }

        if src.exists() {
            std::fs::copy(&src, &tmp)
                .map_err(|e| anyhow::anyhow!("复制 {} 失败: {}", path, e))?;
            std::fs::rename(&tmp, &dst)
                .map_err(|e| anyhow::anyhow!("原子写入 {} 失败: {}", path, e))?;
            synced += 1;
        }
    }

    // 更新本地 manifest
    let mut local_manifest = crate::manifest::DataManifest::load(
        &data_dir.join("data_manifest.json")
    ).unwrap_or(DataManifest { data_version: 0, files: HashMap::new() });

    // 将选中文件的条目更新为 official 版本
    for path in selected_files {
        if let Some(entry) = official_manifest.files.get(path) {
            local_manifest.files.insert(path.clone(), entry.clone());
        }
    }
    local_manifest.data_version = official_manifest.data_version;
    local_manifest.save(&data_dir.join("data_manifest.json"))?;

    // 清理 .official
    std::fs::remove_dir_all(&official_dir)?;

    tracing::info!("Synced {} files, .official removed", synced);

    Ok(ResourceSyncResult {
        success: true,
        files_synced: synced,
        message: format!("成功同步 {} 个文件", synced),
    })
}

/// 递归复制目录（用于 seed）
fn copy_dir_recursive(src: &Path, dst: &Path) -> anyhow::Result<()> {
    if !dst.exists() {
        std::fs::create_dir_all(dst)?;
    }
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}
```

#### 4.3 修改 `src-tauri/src/lib.rs`

```rust
mod resource_sync;

// setup 中注册:
app.manage(resource_sync::ResourceSyncState::default());

// invoke_handler 中注册:
resource_sync::check_resource_sync,
resource_sync::apply_resource_sync,
```

### Step 5: 前端改造

#### 5.1 新建 `ResourceSyncDialog.vue`

**位置**: `src/components/ResourceSyncDialog.vue`

**参考**: `LanSyncDialog.vue` 的设计模式（毛玻璃 Modal + 阶段驱动 UI）

**Props**:

| Prop | 类型 | 说明 |
|---|---|---|
| `visible` | `boolean` | 控制显示 |
| `syncInfo` | `ResourceSyncInfo \| null` | 差异信息 |
| `phase` | `'review' \| 'syncing' \| 'complete' \| 'error'` | 阶段 |
| `errorMessage` | `string` | 错误信息 |

**Emits**: `close`, `apply(selectedFiles: string[])`

**UI 结构**:

```
┌──────────────────────────────────────┐
│  🔄 数据资源同步                      │
│  v42 → v45                          │
│                                      │
│  ┌──────────────────────────────┐   │
│  │ 共 12 个变更  已选 5  12.3MB   │   │
│  │ [全选] [全不选]               │   │
│  ├──────────────────────────────┤   │
│  │ ☑ backgrounds/              │   │
│  │   ☑ 白天.webp  +新增  2.1MB  │   │
│  │   ☑ 夜晚.webp  ~修改  1.8MB  │   │
│  │ ☑ characters/               │   │
│  │   ☐ 诺一钦灵/avatar/新表情.webp│   │
│  │           +新增  0.5MB       │   │
│  │   ☑ 风雪/settings.yml ~修改  │   │
│  └──────────────────────────────┘   │
│                                      │
│        [取消]    [同步选中文件]        │
└──────────────────────────────────────┘
```

**功能要点**:
- 按目录分组展示，目录有父级 checkbox（全选/全不选该目录）
- 每行文件: checkbox + 路径 + 变更标签（绿色"新增"/琥珀色"修改"）+ 文件大小
- 顶部统计栏实时更新
- 同步中显示进度条
- 完成/失败显示结果

#### 5.2 修改 `useUpdater.ts`

**移除**:
- `dataInfo` / `dataProgress` refs
- `applyDataUpdate()` 方法
- `data-update-progress` / `data-update-complete` 事件监听
- `checkForUpdates()` 中的数据更新检查分支

**新增**:
```typescript
// 资源同步相关
const resourceSyncInfo = ref<ResourceSyncInfo | null>(null)
const resourceSyncPhase = ref<'idle' | 'review' | 'syncing' | 'complete' | 'error'>('idle')
const resourceSyncError = ref('')

async function checkResourceSync(): Promise<boolean> {
  const info = await invoke<ResourceSyncInfo>('check_resource_sync')
  resourceSyncInfo.value = info
  if (info.available) {
    resourceSyncPhase.value = 'review'
    return true
  }
  return false
}

async function applyResourceSync(selectedFiles: string[]): Promise<void> {
  resourceSyncPhase.value = 'syncing'
  try {
    const result = await invoke<ResourceSyncResult>('apply_resource_sync', {
      selectedFiles,
    })
    if (result.success) {
      resourceSyncPhase.value = 'complete'
    } else {
      resourceSyncPhase.value = 'error'
      resourceSyncError.value = result.message
    }
  } catch (e) {
    resourceSyncPhase.value = 'error'
    resourceSyncError.value = String(e)
  }
}

function resetResourceSync() {
  resourceSyncInfo.value = null
  resourceSyncPhase.value = 'idle'
  resourceSyncError.value = ''
}
```

**简化 `checkForUpdates()`**:
```typescript
async function checkForUpdates(): Promise<boolean> {
  phase.value = 'checking'
  try {
    const update = await check()
    if (update) {
      appVersion.value = update.version
      appReleaseNotes.value = update.body || ''
      phase.value = 'app-update-available'
      return true
    }
  } catch (e) {
    console.debug('[Updater] check skipped:', String(e).slice(0, 80))
  }
  phase.value = 'idle'
  return false
}
```

#### 5.3 修改 `SettingsText.vue`

"版本更新"菜单项改为：

```
┌──────────────────────────────────────┐
│  🔄 版本更新                          │
│                                      │
│  程序版本    v0.4.6                  │
│  数据版本    v42                     │
│  最新程序    v0.4.7  [检查更新]       │
│  最新数据    v45     [同步数据]       │
│                                      │
│  UpdateDialog (程序更新提醒)          │
│  ResourceSyncDialog (数据同步)        │
└──────────────────────────────────────┘
```

- 程序版本: `getVersion()` 获取
- 数据版本: 从 `data/data_manifest.json` 读取（启动时调用 `invoke` 或在 Rust 侧提供命令）
- "检查更新" → `updater.checkForUpdates()`
- "同步数据" → `updater.checkResourceSync()` → 打开 `ResourceSyncDialog`

新增一个简单的 Rust 命令获取本地数据版本：

```rust
#[tauri::command]
fn get_data_version() -> Result<u64, String> {
    let data_dir = crate::init::static_copy::get_data_dir();
    let manifest_path = data_dir.join("data_manifest.json");
    if manifest_path.exists() {
        let manifest = crate::manifest::DataManifest::load(&manifest_path)
            .map_err(|e| format!("{}", e))?;
        Ok(manifest.data_version)
    } else {
        Ok(0)
    }
}
```

#### 5.4 修改 `App.vue`

**简化启动检查**:
```typescript
async function checkUpdatesOnStartup() {
  setTimeout(async () => {
    try {
      const hasUpdate = await updater.checkForUpdates()
      if (hasUpdate) {
        showUpdateDialog.value = true
      }
    } catch {
      // 静默失败
    }
  }, 3000)
}
```

不再涉及数据更新绑定。`UpdateDialog` 仅用于程序更新通知。

### Step 6: 清理旧模块

1. **删除** `src-tauri/src/data_update/` 整个目录（3 个文件）
2. **修改** `src-tauri/src/lib.rs`:
   - 移除 `mod data_update;`
   - 移除 `data_update::DataUpdateState` 管理
   - 移除 `data_update::check_data_update` / `data_update::apply_data_update` 命令注册
3. `cargo check` 验证

### Step 7: 更新 CI

**文件**: `.github/workflows/release.yml`

**变更**:
1. 移除 `generate-data` job
2. 移除各构建 job 中 `needs: generate-data` 和 `Download data-artifacts` 步骤
3. 确保 `checkout` 步骤包含:
   ```yaml
   - uses: actions/checkout@v4
     with:
       lfs: true   # 拉取 LFS 资源文件
   ```
4. 设置 `DATA_VERSION` 环境变量（传递给 `beforeBuildCommand` 中的脚本）:
   ```yaml
   env:
     DATA_VERSION: ${{ github.run_number }}
   ```
5. 移除 Windows job 中上传 `data_manifest.json` / `data_files.zip` 资产的步骤
6. `tauri-apps/tauri-action@v0` 保留 `includeUpdaterJson: true`

**多平台 LFS 注意**: macOS 和 Linux build jobs 也需 `lfs: true`，但可精简为仅拉取 `icons/` 目录（构建不需要完整 `data/`）。但 `bundle.resources` 在构建时需要 `data/game_data/`、`data/third_party/`、`data/data_manifest.json` 这些文件存在。

实际上，`data/` 下的资源文件在构建时由 Tauri 的 bundler 打包。构建机器上**必须有这些文件**。所以 `lfs: true` 是必需的（或至少拉取对应路径）。

可选优化：使用 sparse checkout 仅拉取需要的 LFS 路径。

### Step 8: 移除 `generate-data-manifest.js` 的冗余

`scripts/generate-data-manifest.js` 保留，但：
- 它同时也是 `beforeBuildCommand` 的一部分（`pnpm tauri build` 自动调用）
- Android 有自己的 `prepare-bundled-resources.mjs`

这两个脚本功能类似但独立。保持现状即可，不需要合并。

---

## 5. 文件变更清单

### 5.1 新建文件

| 文件 | 说明 |
|---|---|
| `src-tauri/src/manifest/mod.rs` | 公共 manifest 类型、diff、load/save、compute_sha256 |
| `src-tauri/src/resource_sync/mod.rs` | Tauri commands + ResourceSyncState |
| `src-tauri/src/resource_sync/sync.rs` | seed_full_from_official + apply_selected_files |
| `src/components/ResourceSyncDialog.vue` | 数据同步差异查看 Modal |

### 5.2 修改文件

| 文件 | 变更 |
|---|---|
| `src-tauri/src/lib.rs` | +`mod manifest` +`mod resource_sync` -`mod data_update`; 注册新命令和状态 |
| `src-tauri/tauri.conf.json` | +`bundle.resources`; 改 `targets: "nsis"`; 改 `beforeBuildCommand` |
| `src-tauri/src/lan_sync/manifest.rs` | `use data_update::manifest` → `use crate::manifest` |
| `src-tauri/src/lan_sync/server.rs` | 同上 |
| `src-tauri/src/lan_sync/messages.rs` | 同上 |
| `src-tauri/src/init/static_copy.rs` | `seed_data_dir()` 桌面端：首次全量 seed + 删除 .official |
| `scripts/generate-data-manifest.js` | +`DATA_VERSION` 环境变量; +排除 `third_party/` |
| `src/composables/useUpdater.ts` | -数据更新逻辑; +`checkResourceSync` / `applyResourceSync`; 简化 `checkForUpdates` |
| `src/components/settings/pages/SettingsText.vue` | 更新"版本更新"区域: 程序版本 + 数据版本 + 两个按钮 |
| `src/App.vue` | 简化启动检查（仅 app 更新） |
| `.github/workflows/release.yml` | -`generate-data` job; -`data-artifacts`; +`DATA_VERSION` env; +`lfs: true` |
| `src-tauri/Cargo.toml` | 版本号适时更新 |

### 5.3 删除文件

| 文件 | 说明 |
|---|---|
| `src-tauri/src/data_update/mod.rs` | 旧的 HTTP 更新命令 |
| `src-tauri/src/data_update/manifest.rs` | 类型已移至 `crate::manifest` |
| `src-tauri/src/data_update/sync.rs` | HTTP 下载 + zip 解压 |

### 5.4 不受影响

| 文件 | 说明 |
|---|---|
| `src-tauri/src/lan_sync/` (其他文件) | 仅 use 路径变更 |
| `scripts/prepare-bundled-resources.mjs` | Android 构建脚本 |
| `src-tauri/src/api/` | 无变更 |
| `docs/android/migration.md` | 无变更 |

---

## 6. 验证清单

- [ ] `cargo check` — 所有 Rust 编译通过
- [ ] `pnpm type-check` — TypeScript 类型检查通过
- [ ] `pnpm tauri build` — 构建成功，NSIS installer 包含 `.official/` + `third_party/`
- [ ] 安装后首次启动: seed 正确执行，`.official/` 被删除，`.seeded` 被创建
- [ ] 安装后首次启动: `third_party/` 文件在 `data/third_party/` 下正确存在
- [ ] 自定义文件（data/ 下非 manifest 中的文件）不被触碰
- [ ] 设置页: 正确显示程序版本 + 数据版本
- [ ] 设置页"同步数据": 差异检测正确（仅 add + modify，无 delete）
- [ ] 设置页"同步数据": 用户可勾选文件，选择同步后正确复制
- [ ] 同步完成后 `.official/` 被删除
- [ ] App 更新后: `.official/` 重新出现，版本号对比正确
- [ ] Android `pnpm android:build` 不受影响
