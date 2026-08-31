# Android 移植记录

本文档记录了 LingChat 从 Windows 桌面端移植到 Android (Tauri v2) 的过程，
重点说明资源打包方案的来龙去脉、当前实现细节，以及未来改进方向。

## 整体架构

```
开发机 (Windows)                         Android 设备
─────────────                           ─────────────
data/                                   /data/user/0/com.noiq.ling_chat/
  game_data/                              ├── game_database.db
  third_party/                            ├── .seeded
                                          ├── data_manifest.json
pretuari 脚本                              ├── game_data/
  │                                        │   ├── backgrounds/
  │  git ls-files data/                    │   ├── characters/
  │  + data/third_party/                   │   ├── musics/
  │         │                              │   └── scripts/
  │         ▼                              └── third_party/
  │   .bundled_build/                          └── emotion_model_19emo/
  │     ├── game_data/...
  │     ├── third_party/...
  │     └── data_manifest.json
  │         │
  │         │ PowerShell Compress-Archive
  │         ▼
  │   data.zip  ──复制──► gen/android/app/src/main/assets/data/data.zip
  │
  └── (结束)
```

## data.zip 的创建流程

### 1. 触发时机

`package.json` 中提供了独立脚本，**不会**在桌面端 `tauri dev` 时自动触发：

```json
"scripts": {
  "android:prepare": "node scripts/prepare-bundled-resources.mjs",
  "android:build": "node scripts/prepare-bundled-resources.mjs && tauri android build --target aarch64"
}
```

仅在构建 Android 时手动运行：`pnpm android:prepare`（单独打包）或
`pnpm android:build`（打包 + 构建一步完成）。桌面端开发完全不受影响。

### 2. 脚本逻辑 (`scripts/prepare-bundled-resources.mjs`)

```
1. 创建临时目录 src-tauri/.bundled_build/

2. git ls-files data/ ──► 仅收集 git-tracked 的文件（.gitignore 是单一真实来源）
   跳过 model.onnx.data (391MB 训练数据，运行时不需要)

3. 始终包含 data/third_party/ 整个目录（包含情绪模型 model.onnx 等，
   虽然被 .gitignore 忽略，但运行时必需）

4. walkDir 扫描所有文件，生成 data_manifest.json（JSON 清单，含 SHA256 + 文件大小）

5. PowerShell Compress-Archive 将所有文件打包为 data.zip

6. 复制 data.zip → src-tauri/gen/android/app/src/main/assets/data/data.zip

7. 删除临时目录 .bundled_build/
```

### 3. data.zip 内部路径结构

```
data.zip
├── data_manifest.json
├── game_data/
│   ├── backgrounds/
│   │   ├── 占卜摊2.webp
│   │   ├── 夜晚.webp
│   │   └── 白天.webp
│   ├── characters/
│   │   ├── 诺一钦灵/
│   │   │   ├── settings.yml
│   │   │   ├── ai模式钦灵.txt
│   │   │   └── avatar/
│   │   │       ├── 伤心.webp
│   │   │       ├── 开心.webp
│   │   │       └── ...
│   │   └── 风雪/
│   │       └── ...
│   ├── musics/
│   │   └── 夜晚音效.mp3
│   └── scripts/
│       └── character/
│           ├── 诺一钦灵/
│           │   ├── 小狼的爱好/
│           │   ├── 想出去玩啦/
│           │   └── 钦灵黄油/
│           └── 风雪/
│               ├── 神秘の魔法药水/
│               ├── 自己做饭才香哦/
│               └── 试着仰望星空/
└── third_party/
    └── emotion_model_19emo/
        ├── model.onnx
        ├── vocab.txt
        └── ...
```

## data.zip 在设备上的存放位置

### APK 内（只读）

```
APK 内部路径:  assets/data/data.zip
Tauri 资源 URI: asset://localhost/data/data.zip
```

`gen/android/app/src/main/assets/` 是 Android 的**原生 assets 目录**，
Gradle/aapt2 自动将其内容打包进 APK。我们没有使用 Tauri 的 `bundle.resources`
机制（该机制已被移除）。

### 解压后（可读写）

```
/data/user/0/com.noiq.ling_chat/
├── data_manifest.json
├── game_data/
│   ├── backgrounds/
│   ├── characters/
│   ├── musics/
│   └── scripts/
├── third_party/
│   └── emotion_model_19emo/
├── .seeded          ← 标记文件，已存在则跳过后续解压
└── game_database.db
```

解压发生在应用首次启动的 `setup` 阶段（`seed_data_dir()` → `seed_via_fs_plugin()`）。

## 运行时解压流程 (`src-tauri/src/init/static_copy.rs`)

```
1. resolve_data_dir()
   ├── mobile (android/ios) → app.path().app_data_dir()
   │                         即 /data/user/0/com.noiq.ling_chat/
   ├── desktop debug        → 项目根目录 data/
   └── desktop release      → exe 旁 data/

2. seed_data_dir()  [仅 mobile 执行，desktop 为 no-op]
   ├── 检查 .seeded + data_manifest.json → 已存在则跳过
   ├── seed_via_fs_plugin():
   │   ├── app.fs().read("asset://localhost/data/data.zip")
   │   │   (tauri-plugin-fs 的 FsExt trait，纯 ASCII 路径)
   │   ├── zip::ZipArchive::new(cursor) 解压到内存
   │   ├── 遍历条目，将 \ 转为 /（Windows zip → Unix 路径）
   │   └── std::fs::write 写入 app_data_dir()
   └── 写入 .seeded 标记

3. 后续启动 → 检查标记 → 跳过解压
```

## 为什么用 zip 而不是 Tauri 官方做法

### 问题根因

Tauri v2 的 `asset://` 协议在 Android 上使用 Android 系统的 `AssetManager`。
对于**含中文（非 ASCII）文件名**的资源，`tauri-plugin-fs` 的 `read()` 方法
无法正确打开：

- 传入原始 UTF-8 中文路径 → `failed to open file`
- 传入百分号编码 (`%E5%8D%A0...`) → AssetManager 查找字面 `%` 文件名，同样失败

**所有中文命名的文件都无法从 `asset://` 读取。** 而本项目有大量中文文件名
（角色名、表情名、背景名、剧本名等）。

### 尝试过的方案

| 方案 | 结果 |
|------|------|
| `bundle.resources` + 逐文件 `app.fs().read()` | ❌ 中文路径失败 |
| 同上 + URI 百分号编码 | ❌ AssetManager 不解码 |
| `include_bytes!` / `include_dir!` | ❌ 用户拒绝嵌入二进制 |
| `tauri-plugin-fs` 读单个 ASCII 文件 | ✅ 可行！ |
| zip 打包 + 单文件读取 | ✅ 可行！ |

### 当前 zip 方案

- 只需从 `asset://` 读取**一个**纯 ASCII 文件：`data.zip`
- 所有中文路径被封装在 zip 内部，由 `zip` crate 处理（正确支持 UTF-8）
- 解压到文件系统后，后续所有 API 使用标准 `std::fs` 读写

### 待改进

Tauri 官方推荐做法应是用 `tauri.conf.json` 的 `bundle.resources` 或
`assetProtocol` 直接提供资源文件，不经过 zip 二次打包。待 Tauri v2
修复 Android 上非 ASCII 资源路径的问题后，可迁移至：

1. 在 `tauri.conf.json` 启用 `bundle.resources` 直接映射资源文件
2. 前端通过 `asset://` 协议或 `convertFileSrc()` 访问资源
3. 后端通过 `app.fs().read()` 或 `app.asset()` 读取
4. 首次启动时用标准方式将必要的可写文件播种到 `app_data_dir()`

## 构建命令

```bash
# 仅打包资源到 Android assets（不编译）
pnpm android:prepare

# 打包 + 构建 release APK（推荐）
pnpm android:build

# 手动构建（需先 android:prepare）
pnpm tauri android build --target aarch64

# 开发模式（需要 adb reverse 或网络可达的 dev server）
pnpm tauri android dev --target aarch64

# 仅 arm64（跳过 armv7/i686/x86_64）— 因为 ort-sys 只提供 aarch64 预构建
# 最终 APK 路径：
#   debug:   src-tauri/gen/android/app/build/outputs/apk/arm64/debug/app-arm64-debug.apk
#   release: src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release-unsigned.apk
#   (release 需用 apksigner 签名后才能安装)
```

## 关键配置文件

| 文件 | 作用 |
|------|------|
| `scripts/prepare-bundled-resources.mjs` | pretuari 脚本，创建 data.zip |
| `src-tauri/src/init/static_copy.rs` | 运行时解压 data.zip、data_dir 解析 |
| `src-tauri/src/init/mod.rs` | 初始化入口，调用 seed_data_dir() |
| `src-tauri/tauri.conf.json` | assetProtocol 已启用，bundle.resources 已移除 |
| `src-tauri/capabilities/default.json` | fs:default + $RESOURCE/** scope |
| `src-tauri/Cargo.toml` | tauri-plugin-fs、zip = "2" |
| `.gitignore` | src-tauri/gen/ 已忽略 |

## 相关 PR

- [#449](https://github.com/SlimeBoyOwO/LingChat/pull/449) — 首个 Android 移植（将资源直接放 `gen/android/app/src/main/assets/data/`，使用 Git LFS）
