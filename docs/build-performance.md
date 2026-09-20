# Rust 增量编译性能优化

> 记录 #603（内置 TTS 推理 GPU 加速，提交 `85ae3757`）之后 Rust 侧增量编译明显变慢的
> 排查结论与优化措施。
>
> 相关文件：`src-tauri/Cargo.toml`、`.cargo/config.toml`、`src-tauri/.cargo/config.toml`、
> `docs/ios-build.md`。

## 1. 结论摘要

**没有拆 TTS crate。** 拆 crate 对增量编译的收益接近零、成本很高，且这个仓库已经拆过
一次又被合回了（见 §3）。真正的瓶颈在构建配置：桌面端一直在产出只有移动端才需要的
产物，并且让依赖也生成调试符号。

优化措施（全部是构建配置，未改任何业务代码）：

| #   | 措施                                                   | 位置                      |
| --- | ------------------------------------------------------ | ------------------------- |
| 1   | `crate-type` 去掉 `staticlib`                          | `src-tauri/Cargo.toml`    |
| 2   | 依赖与 proc-macro 不生成调试符号                       | `src-tauri/Cargo.toml`    |
| 3   | Windows 链接器改用 `rust-lld`                          | 两份 `.cargo/config.toml` |
| 4   | 两份 `.cargo/config.toml` 加同步警告（防止静默不一致） | 两份 `.cargo/config.toml` |

## 2. 效果

### 2.1 每次增量构建写入的产物：2.18 GB → 0.62 GB（-71%）

只要 lib target 被重编（改 `src/` 下任何一行都会），cargo 就会重新产出下列全部文件。
改前 `crate-type = ["staticlib", "cdylib", "rlib"]`，桌面端实际只用到 `rlib`。

| 产物                         | 改前         | 改后             | 说明                           |
| ---------------------------- | ------------ | ---------------- | ------------------------------ |
| `ling_chat_lib.lib`          | **1361 MB**  | **0**            | staticlib，仅 iOS 需要，已移除 |
| `libling_chat_lib.rlib`      | 258 MB       | 258 MB           | 桌面与 bin 自身使用            |
| `ling_chat_lib.dll` + `.pdb` | 31 + 55 MB   | 31 + **29** MB   | cdylib，Android 需要           |
| `ling_chat.exe` + `.pdb`     | 156 + 313 MB | 156 + **150** MB | 桌面可执行文件                 |
| **合计**                     | **~2.18 GB** | **~0.62 GB**     |                                |

依赖侧同样大幅收敛：一次全量重建中 **554 个 rlib 只产生 55 个 PDB**；
`tauri_macros` 的 PDB 从 34 MB 降到 10.4 MB，`rustpython_derive` 从 31.1 MB 降到 6.7 MB
（proc-macro 以 dylib 形式构建，即使关掉调试符号仍会有小体积 PDB）。

### 2.2 墙钟时间

本机（Windows / MSVC，dev profile，改一行源码后重新构建，缓存热）：

| 场景                            | 改前      | 改后                                   |
| ------------------------------- | --------- | -------------------------------------- |
| `cargo check`                   | 32 / 45 s | 2 ~ 9 s                                |
| `cargo build`                   | 40 / 43 s | **21 s**（rust-lld）／23 s（link.exe） |
| `cargo check` 紧跟在 build 之后 | —         | **0.8 s**                              |
| 无改动的 `cargo check`          | 5 s       | 1 s                                    |

`cargo build` 之后再 `cargo check` 几乎免费——Cargo 会复用 build 产出的 rmeta。
反过来（check 之后 build）仍需完整 codegen。

**这些墙钟数字不能全部归因于配置改动**，原因见 §6。

### 2.3 `rust-lld` 的单独 A/B

两种链接器各测 3 轮增量构建，区间不重叠：

| 链接器                  | 3 轮耗时       | 中位数 |
| ----------------------- | -------------- | ------ |
| `rust-lld`              | 20 / 21 / 21 s | 21 s   |
| `link.exe`（MSVC 默认） | 24 / 23 / 23 s | 23 s   |

约 9% 的提升。采纳它的额外理由是**与 CI 对齐**——`dev-build.yml` 的 Windows 任务本来
就通过 `RUSTFLAGS` 设了 `-C linker=rust-lld`（Linux 用 `mold`）。

## 3. 为什么没有拆 TTS crate

初期的设想是把内置 TTS 拆成独立 crate 隔离掉。评估后否决，理由分三类。

### 3.1 机制上无效

增量编译的耗时 = rustc 重编本轮改动的 CGU + **链接**。把 TTS 拆出去之后：

- 改主 crate 代码 → 主 crate 照样要重编、照样要重链。**链接输入一个字节都没少**，
  TTS 代码仍然在同一个二进制里。
- 改 TTS 代码 → 要先重编 TTS crate，再重编并重链主 crate，**反而更慢**。
- 唯一能真正解耦链接的做法是拆成运行时 `dlopen` 的 dylib，与 Tauri 打包、
  Android/iOS 全部冲突，代价高一个数量级。

而且 TTS 只占 **4416 / 64174 行 = 6.9%**。它独占的重依赖只有 `tokenizers`、`esaxx-rs`、
`hound`、`jpreprocess`/`lindera` 词典链和一个多余的 `reqwest 0.12`。**砍不掉的是大头**：
`ort`（情绪分类器 `ai_service/emotion` 与 `ai_service/asr/vad` 也在用）、`ndarray`、
`wgpu`（`utils/gpu_perf.rs`）、`rustpython`（插件系统）、`windows`。

### 3.2 成本很高

- 仓库现在**没有 `[workspace]`**，`src-tauri/Cargo.lock` 有 1048 个包需要迁移。
- 两个 `[patch.crates-io]`（`esaxx-rs`、`jpreprocess-naist-jdic`）**只有 workspace 根生效**，
  且都绑在 TTS 依赖链上。
- `ort-sys` 带 `links = "onnxruntime"`：如果两边各 pin 一次 ort 版本，会直接 hard error。
  历史上为此专门写过 `[patch]`（见 `404f8896` 的 Cargo.toml 注释）。
- `src-tauri/src/utils/device.rs` 是 `sbv2_core` 泄漏到共享层的唯一引用，拆之前必须先把
  `InferenceDevice` 抽成不依赖 `sbv2_core` 的本地类型。
- TTS 目录里有 **21 个 `#[tauri::command]`**、6 个文件 `use tauri::*`、依赖
  `config::settings_store`（`Arc<Store<Wry>>`）——拆出去也不是纯库，得带着 tauri。

### 3.3 已经拆过一次并被主动回退

- `404f8896`（2026-07-25）建了独立 crate `src-tauri/crates/sbv2-local-tts`，
  自带 6827 行 `Cargo.lock` 和独立 `.cargo/config.toml`（后者还写死了另一台机器的
  NDK 绝对路径）。
- `1c93ba2c`（2026-07-30）把它删掉合回主项目，删 8193 行、加 902 行，提交信息是
  「将sbv2适配层代码嵌入项目，**复用大部分工具逻辑**」。

现在 `src-tauri/src/ai_service/tts/local/mod.rs` 第 1 行仍留着
`// Local TTS engine module (formerly the sbv2-local-tts crate, now embedded).`

**如果要重新讨论这个方向，请先读本节。**

## 4. 各项改动的原理

### 4.1 `crate-type` 去掉 `staticlib`

```toml
[lib]
name = "ling_chat_lib"
crate-type = ["cdylib", "rlib"]
```

三种 crate-type 的用途：`staticlib` → iOS（Xcode 链接 `libling_chat_lib.a`）；
`cdylib` → Android（`jniLibs` 引用 `libling_chat_lib.so`）；`rlib` → 桌面端与 bin 自身。

Cargo **不支持**按 target 条件化 `[lib] crate-type`
（实测 `cargo metadata --config 'lib.crate-type=["rlib"]'` 会被静默忽略，cargo 1.96.1）。
所以要么三件套全带上（桌面端白白多写一个 1.3 GB 归档 + 一次 cdylib 链接），
要么去掉暂时用不到的。因为 iOS 当前不构建（`build-ios.yml` 是 `workflow_dispatch`
手动触发，不在 push/PR 上跑），选择先摘掉 `staticlib`。

> ⚠️ **手动 iOS 构建前必须把 `"staticlib"` 加回**，详见 `docs/ios-build.md` 的
> 「构建前必读」。

### 4.2 依赖与 proc-macro 不生成调试符号

```toml
[profile.dev.build-override]
debug = false

[profile.dev.package."*"]
debug = false
```

`build-override` 管 build script 与 proc-macro，`package."*"` 管普通依赖。
自己写的代码仍保留 `[profile.dev] debug = 1`，可以正常下断点。

放在 `src-tauri/Cargo.toml` 而不是 `.cargo/config.toml` 有两个原因：
`package` 覆盖本来就只支持 manifest；且 manifest 的 profile 不受 cwd 影响，
天然绕开「两份 config 要同步」的问题（见 4.4）。

### 4.3 Windows 链接器改用 `rust-lld`

```toml
[target.x86_64-pc-windows-msvc]
linker = "rust-lld"
```

放在 `[target.*]` 表而不是通过 `RUSTFLAGS`：`RUSTFLAGS` 一旦变化会让**所有依赖**的
指纹失效并触发全量重编，而 linker 只在最终链接时生效，不影响各依赖的 rlib。
`rust-lld` 随工具链分发，本机与 CI 都无需额外安装。

### 4.4 两份 `.cargo/config.toml` 的同步警告

Cargo 解析配置是**从 cwd 向上查找**，不是从 manifest 向上。因此：

- `cd src-tauri && cargo build` → 读 `src-tauri/.cargo/config.toml`
- 仓库根 `cargo --manifest-path src-tauri/Cargo.toml`（CI 的做法）→ 读根目录那份

两份都在实际生效，只改一份会让本地与 CI 的编译行为静默不一致。已在两份文件顶部
互相加注释警示。

## 5. 如何复现测量

```bash
# 0) 先确认环境干净（见 §7 的两个坑）
pnpm tauri dev 需已停止；rust-analyzer 应只有 1 个进程

# 1) 基线：无改动时的耗时（应接近 0，确认缓存是热的）
cargo check --manifest-path src-tauri/Cargo.toml

# 2) 改一行源码后重新构建（把 <file> 换成任意 .rs）
#    注意用 ${PIPESTATUS[0]} 取退出码，`| tail` 会把失败伪装成成功
start=$(date +%s)
cargo build --manifest-path src-tauri/Cargo.toml
echo "$(( $(date +%s) - start )) 秒"

# 3) 看每次构建写了哪些产物、多大
ls -la --time-style=+%H:%M:%S src-tauri/target/debug/ | grep ling_chat

# 4) 逐编译单元的耗时分解
cargo build --manifest-path src-tauri/Cargo.toml --timings
#    报告在 src-tauri/target/cargo-timings/
```

测量要点：改一行后测 3 轮取中位数；确认 `ling_chat_lib.lib` 不再出现。

## 6. 方法学说明：哪些数据可归因

改前的基线是在**有干扰**的条件下测的：当时本机同时存在 **3 个 `rust-analyzer` 进程**
（合计约 12 GB 内存）在跑 `cargo check`，`pnpm tauri dev` 也在运行，每次测量都打印
`Blocking waiting for file lock on build directory`。改后的测量是在干净环境下做的。

因此：

| 数据                                   | 可归因性                                                        |
| -------------------------------------- | --------------------------------------------------------------- |
| 产物写入量 2.18 GB → 0.62 GB           | **可明确归因于配置改动**（直接测量）                            |
| 依赖 PDB 缩减（554 rlib → 55 PDB）     | **可明确归因于配置改动**（直接测量）                            |
| `rust-lld` vs `link.exe`：21 s vs 23 s | **可明确归因**（干净环境下各 3 轮 A/B）                         |
| `cargo check` 32~45 s → 2~9 s          | **主要归因于环境清理**（杀掉 3 个 rust-analyzer），不是配置改动 |
| `cargo build` 40~43 s → 21 s           | 两者都有贡献，**未做精确拆分**                                  |

要精确拆分 `cargo build` 的改善，需要在干净环境下重跑一遍带干扰的基线（约 10 分钟）。
当时判断这一步不改变「是否保留这些改动」的结论，故未做。

## 7. 后续维护注意事项

### 7.1 rust-analyzer 进程残留（本机已多次出现）

rust-analyzer 重启时旧进程不会退出，会累积多个实例（父进程是 `rustup.exe`）。
它们都在对同一个 `src-tauri/target` 跑 `cargo check`，后果是：

- 每次手动 `cargo` 都打印 `Blocking waiting for file lock on build directory`
- `cargo build` 可能因 `ling_chat.exe` 被运行中的应用锁住而报
  `failed to remove file ... os error 5`

排查构建变慢前先看进程数，正常应只有 1 个：

```powershell
Get-Process rust-analyzer | Select-Object Id,@{n='GB';e={[math]::Round($_.WorkingSet64/1GB,1)}}
```

多于 1 个就全部 `Stop-Process -Force`，VS Code 会自己重启它需要的那一个。

### 7.2 测量时先停 `pnpm tauri dev`

它既抢构建锁，又会在源码变化时自动重建并重启应用、锁住 `ling_chat.exe`，
导致你手动跑的 `cargo build` 失败。

### 7.3 `cargo ... | tail` 会吞掉退出码

构建失败时看起来仍像成功。判断成败要用 `${PIPESTATUS[0]}`（bash）或
`$LASTEXITCODE`（PowerShell）。

### 7.4 修改 profile 时要同时改两份 config

见 §4.4。新增 `package."*"` / `build-override` 之类的覆盖则放进
`src-tauri/Cargo.toml`。

## 8. 未做但可考虑的方向

以下都**未实施**，仅记录评估结论，需要时再单独讨论：

- **给本地 TTS 加 feature gate**（dev 默认关）：能从 dev 构建里去掉 `sbv2_core`、
  `tokenizers`、`esaxx-rs`、`jpreprocess`/`lindera` 词典链和 `ort/directml`。但成本是给
  21 个 `#[tauri::command]`、`app/setup/*` 装配、`ai_service/types.rs` 的
  `Role.voice_maker` 字段全部加 `cfg`，属于要碰业务代码的改动。
- **桌面构建去掉 `cdylib`**：Cargo 不支持按 target 条件化 crate-type，只能用脚本在构建前
  改写 manifest（工作区会变脏、有忘记切回的风险）。仅当链接仍是主要瓶颈时才值得做。
- **固定 `CARGO_TARGET_DIR`**：本次排查确认 IDE 与 CLI 用的是同一个 target 目录，
  不存在缓存不共享的问题，故不需要。
