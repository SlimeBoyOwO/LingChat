# iOS 构建指南（无签名 IPA）

> 本文档描述 LingChat 的 iOS 支持现状与打包流程。
> **iOS 构建只能在 macOS 上执行**（`tauri ios` 子命令仅存在于 macOS 版 tauri-cli）。

## 现状

- 后端（Rust）已支持 iOS：数据播种走 `data.7z`（与 Android 同一机制，见
  `src-tauri/src/init/static_copy.rs` 的 `seed_via_fs_plugin`）。
- iOS 数据目录 = 沙盒内 **Documents**（`<container>/Documents`），配合
  `src-tauri/Info.ios.plist` 的 `UIFileSharingEnabled` / `LSSupportsOpeningDocumentsInPlace`，
  用户可在系统「文件」App 中直接看到并访问整个 `data/` 目录
  （游戏数据、语音、截图、数据库等）。
- Xcode 工程由 `tauri ios init` 生成（XcodeGen），目标**兼容 iPhone 与 iPad**
  （`TARGETED_DEVICE_FAMILY = "1,2"`，XcodeGen 默认即此值，`scripts/configure-ios-project.sh`
  会显式归一化兜底）。
- **App 图标与 Android 同源**：`configure-ios-project.sh` 每次配置时把
  `src-tauri/icons/ios/`（`tauri icon` 基于 `src-tauri/icons/icon.png` 生成，与
  Android 使用的源图相同）同步到 Xcode 工程 `Assets.xcassets/AppIcon.appiconset/`。
  `gen/apple` 是本地持久产物（gitignored），工程已存在时会跳过 `ios init`，
  不同步图标会停留在旧版本（实测曾出现 iOS 显示旧版 logo、Android 显示新版蓝猫
  图标的不一致）。若同步时提示目录缺失，先执行 `pnpm run init` 生成图标。
- 截图插件（`tauri-plugin-screenshots`）依赖的 `xcap` 不支持 iOS，已在插件内打桩：
  iOS 构建时排除 xcap，所有截图命令返回「不支持」错误（`screenshots:default`
  权限仍可正常解析，capabilities 无需改动）。

## 前置条件（macOS）

```bash
brew install xcodegen
rustup target add aarch64-apple-ios      # 真机目标
rustup target add aarch64-apple-ios-sim  # 模拟器（可选，本流程默认只打真机包）
pnpm install
pnpm run init        # 生成图标 + 下载情绪模型（ONNX）到 data/third_party
```

## 模拟器开发调试（tauri ios dev）

```bash
node scripts/prepare-bundled-resources.mjs 9   # 先生成 data.7z（首次必须，之后资源变更时重跑）
pnpm tauri ios dev "iPhone 17 Pro"             # 设备名可换成任意可用模拟器（xcrun simctl list devices）
```

注意：
- **`tauri ios dev` 前必须先有 `gen/apple/assets/data/data.7z`**（`prepare-bundled-resources.mjs` 生成），
  否则首启播种失败，setup 报错后在 `did_finish_launching` 内触发不可 unwind 的 panic（SIGABRT）。
- 不要给该命令设置相对路径的 `CARGO_TARGET_DIR`（如 `src-tauri/target`）：cargo 会把它解析到
  `src-tauri` 内部（如 `src-tauri/src-tauri/target`），tauri-cli 的文件 watcher 监听到 target 目录
  变化会无限触发重建-重部署循环。默认（不设该变量，产物落在 `src-tauri/target`）即正常。

## 打包无签名 IPA

### 一键脚本

```bash
pnpm run ios:build
```

等价于（分步）：

```bash
bash scripts/configure-ios-project.sh      # 1. init Xcode 工程 + iPhone/iPad 兜底
node scripts/prepare-bundled-resources.mjs 9  # 2. data.7z → gen/apple/assets/data/
pnpm tauri ios build --no-sign             # 3. 构建（beforeBuildCommand 自动跑前端构建）
```

产物：`src-tauri/gen/apple/build/<arch>/LingChat.ipa`（无签名）。

### CI

`.github/workflows/build-ios.yml`（手动触发）在 `macos-latest` 上执行同一流程，
IPA 作为 workflow artifact 上传（保留 7 天）。构建步骤直接复用
`pnpm run ios:build`（`build-ios-unsigned.sh`），与本地流程单一事实来源；
构建相关环境变量（`CI` / `pnpm_config_*` / `CARGO_PROFILE_RELEASE_*`）由脚本
内置设置，workflow 的 `compression` 输入通过 `IOS_BUNDLED_7Z_LEVEL` 透传给
`prepare-bundled-resources.mjs`（默认 9）。

**本地与 CI 产出的等价性**：流程、依赖锁、资源清单（`data.7z` 按 git ls-files）、
图标与前端构建（同一 commit + 同一 lockfile）均一致，IPA 可互换使用。已知且
**无法消除**的非确定性：构建时间戳/UUID、Rust 与 Xcode 工具链版本（CI 的
`rust-toolchain@stable` 与 `macos-latest` 会滚动更新）、以及 Apple 对
Assets.car/签名的重编码。**注意**：本地工作区如有未提交的 `data/` 改动，会被
`prepare-bundled-resources.mjs` 打进本地 IPA（CI 只打包 commit 内容）。

## 侧载安装

无签名 IPA 无法直接双击安装，需要用侧载工具：

- **Sideloadly** / **AltStore**（免费，Apple ID 签名后安装，7 天有效）
- **爱思助手 / 3uTools**（无签名直装，需越狱或开发者模式）
- 企业签名 / TestFlight（需开发者账号）

安装后首次启动需在 设置 → 通用 → VPN 与设备管理 中信任该开发者。

## 关键设计决策

### 1. 为什么数据目录放 Documents 而不是默认的 app_data_dir

- iOS 的 `app_data_dir()` 位于 `Library/Application Support/`，「文件」App 不可见；
- `UIFileSharingEnabled` 只暴露沙盒的 **Documents** 目录；
- 因此 `static_copy.rs` 的 iOS 分支使用 `document_dir()`。

> 注意：Documents 默认会参与 **iCloud 备份**。若担心 `third_party/` 模型（数百 MB）
> 占用备份空间，可后续在启动时对 `data/third_party` 设置 `NSURLIsExcludedFromBackupKey`
> 排除备份（不影响「文件」App 可见性）。

### 2. 为什么 Info.ios.plist 是单独的 plist

`tauri ios build` 每次构建都会**重新合并** Info.plist，来源按序为：
`gen/apple/<app>_iOS/Info.plist`（XcodeGen 生成）→ 版本号 → `src-tauri/Info.plist`（可选）
→ **`src-tauri/Info.ios.plist`（可选，iOS 专属钩子）** → `bundle.ios.info_plist`（配置）。
因此把文件共享键与设备族写进 `Info.ios.plist` 可稳定生效，不随工程再生成而丢失。

### 3. 资源文件怎么进包

| 内容 | 机制 | 落点 |
|---|---|---|
| 游戏数据 + 模型 | `prepare-bundled-resources.mjs` 打包 `data.7z` → `gen/apple/assets/data/data.7z` | `<bundle>/assets/data/data.7z`（folder reference 保留 `assets/` 目录名），首启解压到 Documents |
| 桌面 `.official` 资源 | `prepare-desktop-resources.mjs` 在移动端（`TAURI_ENV_PLATFORM=ios/android`）**只生成空占位** | 不进包（避免与 data.7z 重复） |

`gen/apple/assets/` 在 project.yml 中是 folder reference（`type: folder`），构建时**连同目录名**拷入
bundle（`assets/data/data.7z` → `<bundle>/assets/data/data.7z`）。
Rust 侧 `seed_via_fs_plugin` 的 iOS 读取路径为 `{resource_dir}/data/data.7z`：
iOS 上 tauri 的 `resource_dir()` 返回的**就是** bundle 内的 `assets/` 目录
（`<bundle>/assets/`，即 folder reference 的落点），不要再拼一层 `assets/`。
Android 同样是 `{resource_dir}/data/data.7z`（APK 的 resource_dir 即 assets 根）。
> 实测记录：曾写成 `{resource_dir}/assets/data/data.7z`，在 iOS 上解析为
> `<bundle>/assets/assets/data/data.7z` 导致首启播种失败、setup 报错并触发
> `did_finish_launching` 内不可 unwind 的 panic（SIGABRT）。CI 只打包不运行，
> 因此该路径错误长期未被发现；`tauri ios dev` 首次真跑才暴露。

> 兼容性说明：`prepare-desktop-resources.mjs` 的 `isMobile` 跳过逻辑对 Android 同样生效，
> Android APK 也会因此去掉重复打包的 `.official` 文件（移动端播种本就只认 data.7z）。

### 4. 前端安全区 / 视口适配（dvw/dvh + env）

iOS 全屏 webview（`viewport-fit=cover`）里 `env(safe-area-inset-*)` 是真实非零值
（iPhone 17 Pro 实测：top 62px / bottom 34px / left,right 0），而桌面/Android 桌面为 0。
前端适配口径（`src/App.vue`、`src/composables/useZoom.ts`）：

- **视口单位统一为动态视口** `dvw/dvh`（替换 `vw/vh`）：iOS 竖屏 `100vh` 不含顶部安全区、
  横屏 `100vw` 含左右安全区，`dvw/dvh` 与视觉视口一致；桌面 `dvw/dvh === vw/vh`，零回归。
- **`#app` 铺满整个视觉视口**（`position: fixed` + `100dvw×100dvh`），**不**整体内缩安全区——
  各屏壁纸/背景因此天然全出血显示（含状态栏与 Home 指示器区域，不会出现上下黑边）。
- **安全区内缩由各边缘元素自行处理**：全局已定义 `--safe-area-inset-*`（`base.css`，回退 env()）
  与工具类 `.pt-safe/.pb-safe/.pl-safe/.pr-safe`（及 `-gap` 变体）；聊天输入区用
  `.main-box { padding-bottom: env(safe-area-inset-bottom) }` 抬离 Home 指示器，
  菜单 Logo（`StartLogo`）用 `top: env(safe-area-inset-top)` 避开状态栏。
- **不要**整体收缩 `#app` 成安全区盒子：会留下裸露黑边（壁纸只画在 #app 内），
  且与已在各 fixed 元素里加过 `var(--safe-area-inset-*)` 的偏移形成**双重内缩**。
- 小屏菜单排布：`StartItem` 在 <768px 允许换行（`max-[767px]:whitespace-normal`）并缩小字号
  （`clamp(26px,4vw,72px)`），否则"剧情模式（在自由模式进入自由模式）"这类长文案
  会横向溢出被裁掉。

> 实测记录：曾把 `#app` 内缩为安全区盒子（`top/env` + `100dvh - insets`），
> 模拟器截图（`xcrun simctl io booted screenshot`）可见屏幕上下各一条黑/透明带——
> 之前无视觉模型、纯靠尺寸推断没有发现；`tauri ios dev` + 截图人工核对才暴露。

## 已知限制

- 无签名 IPA 只能侧载自用，**不能上架 App Store / TestFlight**；
- Windows / Linux 无法生成或构建 iOS 工程（tauri-cli 未提供 ios 子命令）；
- 本仓库在 Windows 侧无法做 iOS 交叉编译验证，首次真机构建请以 macOS 上
  `pnpm run ios:build` 的实际结果为准；
- ort（ONNX Runtime）在 iOS 有官方预编译产物（`aarch64-apple-ios`），
  已确认可编译链接，无需额外配置。

## 踩坑记录

### macOS 自带 bash 3.2 的多字节解析 bug

macOS 自带 `/bin/bash` 是 3.2.57，存在多字节（UTF-8）解析缺陷：
**双引号内 `$VAR` 紧跟多字节字符（如全角逗号 `，`）时，bash 会把多字节字节串
错误并入变量名**，`set -u` 下报 `VAR<乱码>: unbound variable`（即使变量已赋值）。

例：`echo "未找到 $GEN_APPLE，执行..."` 在 bash 3.2 上触发该 bug（GitHub Actions
macOS runner 实测复现；错误信息中变量名后出现 U+FFFD，文件本身经字节级校验为
干净 LF UTF-8，与 CRLF 无关）。

对策：**本仓库的 `.sh` 脚本一律保持纯 ASCII**（注释/输出用英文），
不要在多字节字符后紧跟变量展开。`.gitattributes` 已强制 `*.sh` 为 LF。

### pnpm 11 在 Xcode "Build Rust Code" 阶段的无 TTY 中止

`tauri ios init` 在 pnpm 环境下会把 Xcode 工程的 "Build Rust Code" 脚本阶段
渲染为 `pnpm tauri ... xcode-script ...`（tauri-cli 依据 argv[0] 与
`PNPM_PACKAGE_NAME` 决定 tauri-binary）。pnpm 11 执行脚本前会做依赖校验
（verify-deps-before-run）并自动跑 `pnpm install`；一旦需要清空 node_modules，
在无 TTY 环境（Xcode 脚本阶段）直接中止：
`ERR_PNPM_ABORTED_REMOVE_MODULES_DIR_NO_TTY` → `xcodebuild exit 65`。

要点：
- `CI=true` 与 `pnpm_config_verify_deps_before_run` / `pnpm_config_confirm_modules_purge`
  环境变量（pnpm 11 只认 `pnpm_config_*`/`PNPM_CONFIG_*` 前缀，不认 `PNPM_*`；
  `.npmrc` 里的这两个键会被 pnpm 11 的 `isNpmrcReadableKey` 过滤）实测都无法阻止；
- 根治方案：`scripts/configure-ios-project.sh` 在 init 后 patch pbxproj 的
  shellScript，把开头的 `pnpm tauri ` 替换为
  `node "$PROJECT_DIR/../../../node_modules/@tauri-apps/cli/tauri.js" `
  （`$PROJECT_DIR` = `src-tauri/gen/apple`，上三级到仓库根），
  使 Xcode 脚本阶段直接经 node 调用 tauri CLI，完全绕开包管理器。
  `tauri ios build` 的 `synchronize_project_config` 不会重写 shellScript，patch 持久生效。

### 本地 gen/apple 残留被 folder reference 整包（包体膨胀）

`gen/apple/` 被 `.gitignore` 忽略（本地产物），其中 `gen/apple/assets/data/` 以
folder reference（`type: folder`）参与资源阶段——**目录里有什么就进什么包**。
若此前构建/实验把 `.official` 真实文件或 `third_party` 模型目录留在了
`gen/apple/assets/data/` 下，会与 `data.7z` 重复进包（实测 IPA 膨胀到 216 MB，
本应约 172 MB），且桌面 `.official` 内容重复出现。

对策：`scripts/prepare-bundled-resources.mjs` 的 iOS 部署分支与 Android 分支对齐，
**先清空 `gen/apple/assets/data/` 再写入**，确保 bundle 内的 `assets/data/` 只含
`data.7z`。CI 每次全新生成工程、无此问题，但本地反复构建时必须通过该脚本部署资源。

### 本地并发 xcodebuild 会互相破坏 DerivedData

`tauri ios build` / `tauri ios dev` / 手动 `xcodebuild` 共享同一个
`~/Library/Developer/Xcode/DerivedData/ling_chat-*`。多个 xcodebuild 并发（如
`tauri ios dev` 挂着又跑 `tauri ios build`）会导致 `build.db` 磁盘 I/O 错误与
clang `.resp` 响应文件丢失（`** ARCHIVE FAILED **`）。本地排查顺序：确认无残留
`tauri.js`/`xcodebuild` 进程 → `rm -rf ~/Library/Developer/Xcode/DerivedData/ling_chat-*`
→ 重跑。CI 为全新环境，不会触发。

### 已知但未处理的链接警告

`Externals/arm64/release/libapp.a`（ORT 相关静态库）按 iOS 15.1 编译，而工程
deployment target 为 14.0：`ld: warning: object file ... was built for newer 'iOS'
version (15.1) than being linked (14.0)`。当前仅警告、构建不受影响；若未来需要
保证 iOS 14 实机运行，应把 deployment target 统一提升到 15.1（影响面：XcodeGen
工程每次 `tauri ios init` 后需重新设定，需在 configure 脚本中兜底）。

## 构建验证

- 本仓库的 iOS 构建已在 GitHub Actions（`build-ios.yml`，macOS arm64 runner）上
  **实测通过**：配置工程 → 打包 `data.7z` → `tauri ios build --no-sign` 产出无签名
  IPA（`src-tauri/gen/apple/build/arm64/LingChat.ipa`，约 170 MB，含前端 + Rust +
  情绪模型 data.7z），并作为 workflow artifact `lingchat-ios-unsigned` 上传。
- **本地实测结论（macOS 26.6.1 + Xcode 26.6 + iPhone 17 Pro 模拟器 iOS 26.5）**：
  - 打包流程 `pnpm run ios:build` 全链路通过，IPA 约 **172 MB**；
  - `tauri ios dev` 安装到模拟器后 app 正常启动：主菜单完整显示（壁纸插画 /
    Logo / 全部菜单项），安全区适配正确（状态栏与 Home 指示器无遮挡）；
  - `data.7z` 首启播种成功：`Documents/` 下 `game_data/`（3 个角色、背景、剧本、
    技能，135 个文件）+ `data_manifest.json`（147 条）+ `third_party/` 模型齐全；
  - IPA 内 `assets/data/data.7z`（74 MB）与 Rust 侧读取路径
    `{resource_dir}/data/data.7z` 对应，bundled `data/.official/` 仅含空占位。
- 实测过程中修复的问题（详见踩坑记录）：
  1. 本地 `gen/apple/assets/data/` 旧残留（`.official` 真实文件 + 重复模型）被
     folder reference 整体打进包体，IPA 膨胀到 216 MB → `prepare-bundled-resources.mjs`
     的 iOS 部署改为**先清空再写入**（与 Android 分支一致），修复后 172 MB；
  2. `Info.ios.plist` 的 `UIDeviceFamily` 与 `TARGETED_DEVICE_FAMILY` build setting
     冲突（xcodebuild 警告且该键会被覆盖）→ 从 `Info.ios.plist` 移除，设备族统一
     由 `configure-ios-project.sh` 归一化；
  3. `build-ios-unsigned.sh` 内置 `CI=true` + `pnpm_config_*` 环境变量，本地直跑
     不再因 pnpm 11 无 TTY 依赖校验中止（此前只有 CI 步骤设置，本地脚本缺失）。

