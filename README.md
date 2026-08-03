# LingChat 对话框外观自定义补丁

## ⚠️ 重要前提

**这个补丁是源码补丁，不能直接覆盖到 `G:\Lingchat\ling_chat.exe`。**

LingChat 是用 **Tauri + Vite + TypeScript** 构建的桌面应用。`ling_chat.exe` 是已编译的二进制文件，无法直接修改。要使用本补丁，你需要：

1. 从 [GitHub](https://github.com/myh1011/LingChat) 克隆完整源码
2. 把本补丁的 3 个文件覆盖到对应路径
3. 重新构建出新的 `LingChat.exe`（约需 10-30 分钟）

构建需要的工具：
- **Node.js** ≥ 18（[下载](https://nodejs.org)）
- **Rust** 工具链（[下载](https://rustup.rs)）
- **pnpm**（`npm install -g pnpm`）
- **Tauri CLI 依赖**（Windows：Microsoft C++ Build Tools + WebView2）

## 改动概览

本补丁在「设置 → 背景」标签页里新增了一个**「对话框外观」**区块，允许自定义主界面底部对话框（消息显示 + 输入框区域）的外观。

### 新增的 6 项可调参数

| 参数 | 范围 | 默认 | 说明 |
|------|------|------|------|
| 自定义背景图 | 图片文件 ≤ 2MB | 无 | 上传一张图片作为对话框底图（base64 存到 localStorage） |
| 背景透明度 | 0 - 100% | 70% | 控制渐变底色的不透明度 |
| 背景模糊 | 0 - 20 px | 2 px | 毛玻璃模糊强度（0 = 关闭） |
| 圆角大小 | 0 - 32 px | 8 px | 对话框的圆角 |
| 渐变底色 | HEX 颜色 | `#000e27`（深蓝） | 选色器 + 文本框双输入 |
| 文字颜色 | HEX 颜色 | `#ffffff` | 对话框内文字色 |

所有设置**自动保存**到 `localStorage`，下次启动自动恢复。底部还有"实时预览"和"全部重置为默认"按钮。

## 文件清单

```
lingchat-patch/
├── dialog-appearance.patch         # 535 行 unified diff 补丁（一键应用）
└── src/
    ├── components/
    │   ├── settings/pages/
    │   │   └── SettingsBackground.vue    # 新增「对话框外观」区块
    │   └── game/standard/
    │       └── GameDialog.vue            # 改造主界面对话框使用动态样式
    └── stores/modules/settings/
        └── index.ts                      # 新增 6 个 display 设置字段
```

## 使用方法

### 方法 A：用 unified diff 一键应用（推荐）

```bash
# 在克隆好的 lingchat 源码根目录执行
cd path/to/LingChat
git apply --check ../dialog-appearance.patch   # 先 dry-run 验证
git apply ../dialog-appearance.patch
```

如果 `git apply` 报错（可能因为源版本不一致），可以改用方法 B。

### 方法 B：手动覆盖 3 个文件

把 `lingchat-patch/src/` 下的 3 个文件复制到源码对应位置：

```
源码路径                                                    ←  补丁路径
src/components/settings/pages/SettingsBackground.vue       ←  lingchat-patch/src/components/settings/pages/SettingsBackground.vue
src/components/game/standard/GameDialog.vue                ←  lingchat-patch/src/components/game/standard/GameDialog.vue
src/stores/modules/settings/index.ts                      ←  lingchat-patch/src/stores/modules/settings/index.ts
```

## 重新构建

```bash
# 1. 安装依赖
pnpm install

# 2. 开发模式验证（热重载，推荐先跑这个）
pnpm dev

# 3. 正式构建（产出新的 LingChat.exe）
pnpm tauri build
```

构建产物路径（Windows）：`src-tauri/target/release/bundle/`

## 兼容性

- 适用于 LingChat v0.4.7（当前 `main` 分支）
- 旧版本：若 settings store 的 `display` 类别下缺少 6 个新字段，**会自动用默认值填充**（`persist.ts` 用了 `deepMerge` 合并），不会破坏现有设置
- 已有数据迁移：用户已有的 `lingchat-settings` localStorage 数据完全保留

## 实现原理（技术向）

1. **持久化**：在 Pinia `useSettingsStore` 的 `display` 类别下新增 6 个字段，通过插件 `persist: true` 自动同步到 `localStorage`。
2. **响应式**：`GameDialog.vue` 用 `computed` 把这些字段组装成 CSS 内联样式 `background / backdropFilter / borderRadius / color`，配合 `v-if` 控制背景图。
3. **图片存储**：用户上传的图片用 `FileReader.readAsDataURL` 转 base64，存到 settings 字段。优点是无需后端命令、跨重启保留；限制是 ≤ 2MB。
4. **渐变色**：`hexToRgba` 工具函数把 HEX + 透明度转成 `rgba()`，再组装 `linear-gradient`。

## 已知限制

- **图片不能 > 2MB**（base64 后约 2.7MB，localStorage 上限 5-10MB）
- **没有做实时缩放/裁剪**，如果图片比例不对，会被 `object-cover` 裁剪
- **不支持 GIF 动图**（会被作为静态图显示）

## 反馈

如有问题，请附带：
- LingChat 版本号
- 浏览器/系统信息
- 复现步骤和截图
