# Android 移植记录

本文档记录了 LingChat 从 Windows 桌面端移植到 Android (Tauri v2) 的过程，
重点说明资源打包方案的来龙去脉、当前实现细节，以及未来改进方向。

> **现状（2026-10 合入 `upstream/dev` 后）**：上游已把移动端资源方案改成
> `data.7z`（7z 打包 + `sevenz_rust2` 进程内解压，代码见
> `crates/ling-chat-main/src/data_dir.rs`），安装目录也改为应用专属外部存储
> `/storage/emulated/0/Android/data/com.noiq.lingchat/files`。
> 本文档下面关于 `data.zip` / `static_copy.rs` / `ling_chat` 包名的描述属于**合入前**的
> 历史实现，只作背景参考；与当前实现相关的坑见文末「已知问题（Android 侧）」。

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

| 方案                                          | 结果                   |
| --------------------------------------------- | ---------------------- |
| `bundle.resources` + 逐文件 `app.fs().read()` | ❌ 中文路径失败        |
| 同上 + URI 百分号编码                         | ❌ AssetManager 不解码 |
| `include_bytes!` / `include_dir!`             | ❌ 用户拒绝嵌入二进制  |
| `tauri-plugin-fs` 读单个 ASCII 文件           | ✅ 可行！              |
| zip 打包 + 单文件读取                         | ✅ 可行！              |

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

### 构建依赖

| 依赖                   | 用途                                                                 |
| ---------------------- | -------------------------------------------------------------------- |
| Android NDK            | `pnpm android:*` 会自动解析并导出 `ANDROID_NDK_HOME`（见下）         |
| cmake（+ ninja，可选） | 编译 libopus（参考音频的 Opus 解码，`opus` crate 的 bundled 静态库） |

`ANDROID_NDK_HOME` 由 `scripts/resolve-android-ndk.mjs` 解析：优先用已设置的值，其次是
`NDK_HOME`、`$ANDROID_HOME/ndk`、`$ANDROID_SDK_ROOT/ndk` 和默认安装位置里版本最高的一个。
之所以要显式导出，是因为 libopus 的 `build.rs` 只认 `ANDROID_NDK_HOME`，而 Tauri CLI
只处理 `ANDROID_HOME` / `NDK_HOME`；缺了它 libopus 会用宿主编译器编出 x86_64 的目标文件，
链接阶段直接失败。桌面端构建只需要 cmake 在 PATH 上。

构建完 `libopus.a` 会被静态链进 `libling_chat_lib.so`，**不需要**往 `jniLibs` 里加新的 `.so`
（`jniLibs` 里仍然只有 sherpa-onnx / onnxruntime）。

## 关键配置文件

| 文件                                    | 作用                                          |
| --------------------------------------- | --------------------------------------------- |
| `scripts/prepare-bundled-resources.mjs` | pretuari 脚本，创建 data.zip                  |
| `src-tauri/src/init/static_copy.rs`     | 运行时解压 data.zip、data_dir 解析            |
| `src-tauri/src/init/mod.rs`             | 初始化入口，调用 seed_data_dir()              |
| `src-tauri/tauri.conf.json`             | assetProtocol 已启用，bundle.resources 已移除 |
| `src-tauri/capabilities/default.json`   | fs:default + $RESOURCE/\*\* scope             |
| `src-tauri/Cargo.toml`                  | tauri-plugin-fs、zip = "2"                    |
| `.gitignore`                            | src-tauri/gen/ 已忽略                         |

## 已知问题（Android 侧）

- **sherpa-onnx 的 kokoro int8 模型在 arm64 Android 上不可用**：会输出纯静音或钉死的
  垃圾样本（上游 [k2-fsa/sherpa-onnx#3754](https://github.com/k2-fsa/sherpa-onnx/issues/3754)，
  未量化版本不受影响）。实机对比（同一段代码、同一台设备）：

  | 模型                      | 输出                                |
  | ------------------------- | ----------------------------------- |
  | `kokoro-int8-...`         | 4.350s 全静音（峰值 0）             |
  | `kokoro-...`（fp32）      | 中文 2.648s / 英文 2.815s，音量正常 |
  | `matcha-icefall-zh-baker` | 2.357s 正常                         |

  因此内置模型清单在 Android 上提供未量化版本（HuggingFace `csukuangfj/kokoro-multi-lang-v1_1`，
  下载走 `hf-mirror.com`），桌面端保留体积更小的 int8；适配器对 Android 上的 int8
  kokoro 直接报错并提示换模型，已安装的 int8 版本在设置页会被标注「当前平台不可用」。

- **NNAPI 实际不会生效**：预编译的 `sherpa-onnx-v1.13.5-android.tar.bz2` 基于 API level 21
  构建，即使把 provider 设为 `nnapi`，C 库也会打印
  `Android NNAPI requires API level >= 27. Current API level 21 Fallback to cpu!`。
  日志里的「使用推理后端: nnapi」只是我们请求的值，不是实际后端（实测输出与 cpu 一致）。

- **APK 内的 `data.7z` 必须以 `Stored`（不压缩）方式存放**：aapt2 默认会对
  `assets/data/data.7z` 做 DEFLATE，而 `tauri-plugin-fs` 读 APK 内资源的方式是
  「先 `AssetManager.openFd()`（压缩资源拿不到 fd）→ 退化成正把整个文件
  `assets.open(path, ACCESS_BUFFER)` 拷进 `cacheDir`」。这条路在 Android ≤ 15 上能过
  （首次启动多花 ~25s 拷 84MB），但 **Android 16（SDK 36）会直接抛异常**：表现为「启动即崩、
  `setup: 数据层 bootstrap 失败: failed to read data.7z`」，而同一个 APK 在 Android 15
  手机上正常（实测平板 TB710FU / Android 16 + 手机 2201123C / Android 15）。
  修法是在 `src-tauri/gen/android/app/build.gradle.kts` 的 `android {}` 里加：

  ```kotlin
  androidResources {
      noCompress += "7z"   // data.7z 本身已压缩，再 DEFLATE 毫无收益
  }
  ```

  注意 `src-tauri/gen/android/**` 是**被 git 跟踪**的，`tauri android init` 不带 `--force`
  时不会覆盖它，所以这个改动能留下来（`pnpm android:devbuild` 也不会重新 init）。

- **`tauri-plugin-fs` 必须 ≥ 2.7.1**：2.5.1 的 Android 实现在 `openFd()` 成功时只把裸 fd 交给
  Rust，**丢掉了 `AssetFileDescriptor.startOffset`**，于是 Rust 从 APK 文件偏移 0 开始读，
  读到的是 ZIP 头 → `failed to extract data.7z: BadSignature([80, 75, 3, 4, 0, 0])`。
  上游 2.7.x 已改为「APK 内资源一律先拷到 `cacheDir` 再给 fd」（其 Kotlin 注释里写明原因）。
  所以两个条件必须**同时**满足：

  | `data.7z` 存放方式     | `tauri-plugin-fs` | 结果                                                                              |
  | ---------------------- | ----------------- | --------------------------------------------------------------------------------- |
  | DEFLATE（aapt2 默认）  | 2.5.1             | Android ≤ 15 可启动（首次启动拷 84MB 到 cacheDir，~25s）；**Android 16 启动即崩** |
  | Stored（`noCompress`） | 2.5.1             | 读到 APK 自身的 ZIP 头 → `BadSignature` 崩溃                                      |
  | Stored（`noCompress`） | 2.7.1             | 正常                                                                              |

  升级方式（Rust crate 与 NPM 包的 major/minor **必须一致**，
  否则 `tauri android build` 直接报 `Found version mismatched Tauri packages`）：

  ```bash
  cargo update -p tauri-plugin-fs --precise 2.7.1    # 连带 tauri 2.11.1 → 2.12.4
  pnpm update @tauri-apps/api @tauri-apps/plugin-fs  # JS 侧跟上（package.json 范围无需改）
  ```

  自己重新签名 APK 时也要注意：用 `zipalign` + `apksigner` 这类**只签名、不重新打包**的工具，
  别用会重新压缩资源的工具，否则 `data.7z` 可能被重新 DEFLATE，又回到 Android 16 崩的那一档。

## 相关 PR

- [#449](https://github.com/SlimeBoyOwO/LingChat/pull/449) — 首个 Android 移植（将资源直接放 `gen/android/app/src/main/assets/data/`，使用 Git LFS）
